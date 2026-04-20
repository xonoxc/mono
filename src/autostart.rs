use log::{debug, info, warn};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SERVICE_NAME: &str = "screen-time-tracker";
const DESKTOP_FILE_NAME: &str = "screen-time-tracker.desktop";

pub fn get_autostart_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("autostart"))
}

pub fn get_systemd_user_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let systemd_dir = home.join(".config").join("systemd").join("user");
    Some(systemd_dir)
}

pub fn is_autostart_enabled() -> bool {
    if let Some(dir) = get_autostart_config_dir() {
        if dir.join(DESKTOP_FILE_NAME).exists() {
            return true;
        }
    }
    if let Some(dir) = get_systemd_user_dir() {
        if dir.join(format!("{}.service", SERVICE_NAME)).exists() {
            return true;
        }
    }
    false
}

pub fn setup_autostart() -> Result<(), Box<dyn std::error::Error>> {
    info!("Setting up autostart...");

    let systemd_ok = try_setup_systemd();
    if systemd_ok.is_ok() {
        info!("Autostart configured via systemd");
        return Ok(());
    }

    let xdg_ok = try_setup_xdg_autostart();
    if xdg_ok.is_ok() {
        info!("Autostart configured via XDG autostart");
        return Ok(());
    }

    Err("Failed to setup autostart".into())
}

fn try_setup_systemd() -> Result<(), Box<dyn std::error::Error>> {
    let systemd_dir = get_systemd_user_dir().ok_or("No systemd user dir")?;
    fs::create_dir_all(&systemd_dir)?;

    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get exe path: {}", e))?;

    let service_content = format!(
        r#"[Unit]
Description=Screen Time Tracker - Privacy-first screen time monitoring
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=10
Environment=RUST_LOG=info

[Install]
WantedBy=graphical-session.target
"#,
        exe_path.display()
    );

    let service_path = systemd_dir.join(format!("{}.service", SERVICE_NAME));
    fs::write(&service_path, service_content)?;
    debug!("Wrote systemd service to {}", service_path.display());

    let output = Command::new("systemctl")
        .args(["--user", "enable", &format!("{}.service", SERVICE_NAME)])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            return Ok(());
        }
        warn!(
            "systemctl enable failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn try_setup_xdg_autostart() -> Result<(), Box<dyn std::error::Error>> {
    let autostart_dir = get_autostart_config_dir().ok_or("No autostart dir")?;
    fs::create_dir_all(&autostart_dir)?;

    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get exe path: {}", e))?;

    let desktop_content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Screen Time Tracker
Comment=Privacy-first screen time monitoring daemon
Exec={}
Icon=screen-time-tracker
Terminal=false
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Categories=Utility;Monitor;
"#,
        exe_path.display()
    );

    let desktop_path = autostart_dir.join(DESKTOP_FILE_NAME);
    fs::write(&desktop_path, desktop_content)?;
    debug!("Wrote desktop entry to {}", desktop_path.display());

    Ok(())
}

pub fn remove_autostart() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(dir) = get_systemd_user_dir() {
        let service_path = dir.join(format!("{}.service", SERVICE_NAME));
        if service_path.exists() {
            let _ = Command::new("systemctl")
                .args(["--user", "disable", &format!("{}.service", SERVICE_NAME)])
                .output();
            fs::remove_file(&service_path)?;
            debug!("Removed systemd service: {}", service_path.display());
        }
    }

    if let Some(dir) = get_autostart_config_dir() {
        let desktop_path = dir.join(DESKTOP_FILE_NAME);
        if desktop_path.exists() {
            fs::remove_file(&desktop_path)?;
            debug!("Removed desktop entry: {}", desktop_path.display());
        }
    }

    info!("Autostart removed");
    Ok(())
}