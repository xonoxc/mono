use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mono")
}

pub fn get_consent_file() -> PathBuf {
    get_config_dir().join("consent")
}

pub fn get_daemon_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let daemon = parent.join("mono-tracker");
            if daemon.exists() {
                return daemon;
            }
            // Try looking in PATH
            if let Ok(path) = which::which("mono-tracker") {
                return path;
            }
            // Try ~/.local/bin
            let local_bin = dirs::home_dir()
                .map(|h| h.join(".local/bin/mono-tracker"))
                .filter(|p| p.exists());
            if let Some(path) = local_bin {
                return path;
            }
        }
    }
    PathBuf::from("mono-tracker")
}

pub fn has_consent() -> bool {
    get_consent_file().exists()
}

pub fn set_consent(granted: bool) -> std::io::Result<()> {
    let dir = get_config_dir();
    fs::create_dir_all(&dir)?;

    if granted {
        fs::write(get_consent_file(), "1")?;
        setup_autostart()?;
    } else {
        let _ = fs::remove_file(get_consent_file());
    }
    Ok(())
}

pub fn setup_autostart() -> std::io::Result<()> {
    let config_dir = get_config_dir();
    let systemd_dir = config_dir.join("systemd").join("user");
    let autostart_dir = config_dir.join("autostart");

    let daemon_path = get_daemon_path();
    if !daemon_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Daemon binary not found",
        ));
    }

    // Clean up old services
    let _ = fs::remove_file(systemd_dir.join("mono.service"));
    let _ = fs::remove_file(autostart_dir.join("mono.desktop"));
    let _ = fs::remove_file(systemd_dir.join("screen-time-tracker.service"));
    let _ = fs::remove_file(autostart_dir.join("screen-time-tracker.desktop"));

    let desktop_entry = format!(
        r#"[Desktop Entry]
Type=Application
Name=Mono Screen Time Tracker
Exec={}
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Comment=Privacy-first screen time monitoring daemon
"#,
        daemon_path.display()
    );

    let systemd_service = format!(
        r#"[Unit]
Description=Mono Screen Time Tracker
After=graphical-session.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=10

[Install]
WantedBy=graphical-session.target
"#,
        daemon_path.display()
    );

    if let Some(_user) = std::env::var_os("USER") {
        let _ = fs::create_dir_all(&systemd_dir);
        let _ = fs::create_dir_all(&autostart_dir);

        // XDG autostart as primary method (works across all desktop environments)
        let desktop_path = autostart_dir.join("mono-tracker.desktop");
        let _ = fs::write(&desktop_path, desktop_entry);

        // Systemd as fallback
        let service_path = systemd_dir.join("mono-tracker.service");
        let _ = fs::write(&service_path, systemd_service);

        let _ = Command::new("systemctl")
            .args(["--user", "enable", "mono-tracker.service"])
            .output();
    }

    Ok(())
}

pub fn is_daemon_running() -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 9746)),
        std::time::Duration::from_secs(1),
    )
    .is_ok()
}

#[allow(dead_code)]
pub fn remove_autostart() -> std::io::Result<()> {
    let config_dir = get_config_dir();
    let systemd_dir = config_dir.join("systemd").join("user");
    let autostart_dir = config_dir.join("autostart");

    if let Some(_user) = std::env::var_os("USER") {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "mono-tracker.service"])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "mono.service"])
            .output();

        let _ = fs::remove_file(systemd_dir.join("mono-tracker.service"));
        let _ = fs::remove_file(systemd_dir.join("mono.service"));
        let _ = fs::remove_file(systemd_dir.join("screen-time-tracker.service"));
        let _ = fs::remove_file(autostart_dir.join("mono-tracker.desktop"));
        let _ = fs::remove_file(autostart_dir.join("mono.desktop"));
        let _ = fs::remove_file(autostart_dir.join("screen-time-tracker.desktop"));
    }

    Ok(())
}

pub fn start_daemon() -> std::io::Result<()> {
    use std::process::Stdio;

    if is_daemon_running() {
        return Ok(());
    }

    let daemon_path = get_daemon_path();
    if !daemon_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Daemon not found at {:?}", daemon_path),
        ));
    }

    std::process::Command::new(&daemon_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    for i in 1..=3 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if is_daemon_running() {
            return Ok(());
        }
        eprint!("\rConnecting to daemon... retry {}", i);
    }

    eprintln!("\rFailed to start daemon. Is it installed correctly?");
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to start daemon",
    ))
}
