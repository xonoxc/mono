use std::process::Command;

use super::{WindowInfo, WindowManager};

pub struct KDEWaylandManager;

impl KDEWaylandManager {
    pub fn new() -> Option<Self> {
        // Check for qdbus6 which is the proper tool for KDE Plasma 6
        if which::which("qdbus6").is_ok() || which::which("qdbus").is_ok() {
            Some(Self)
        } else {
            None
        }
    }
}

impl KDEWaylandManager {
    fn get_active_window_via_kwin_script(&self) -> Option<WindowInfo> {
        let qdbus_cmd = if which::which("qdbus6").is_ok() {
            "qdbus6"
        } else {
            "qdbus"
        };

        // Generate a unique marker for this query
        let marker = format!("MONO_WIN_{}", std::process::id());

        let script_content = format!(
            r#"
            var client = workspace.activeWindow;
            if (client) {{
                var result = {{
                    title: client.caption || '',
                    app_id: client.resourceClass || client.appId || ''
                }};
                print('{}:' + JSON.stringify(result));
            }} else {{
                print('{}:{{}}');
            }}
        "#,
            marker, marker
        );

        let script_path = "/tmp/mono_kwin_script.js";
        std::fs::write(script_path, script_content).ok()?;

        // Load the script
        let load_output = Command::new(qdbus_cmd)
            .args([
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting.loadScript",
                script_path,
            ])
            .output()
            .ok()?;

        if !load_output.status.success() {
            return None;
        }

        let script_id = String::from_utf8_lossy(&load_output.stdout)
            .trim()
            .to_string();

        if script_id.is_empty() || script_id == "0" {
            return None;
        }

        // Record time before execution
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();

        // Start the script
        let _ = Command::new(qdbus_cmd)
            .args([
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting.start",
            ])
            .output();

        // Wait for script to execute
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Unload the script
        let _ = Command::new(qdbus_cmd)
            .args([
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting.unloadScript",
                &format!("mono_kwin_script_{}", script_id),
            ])
            .output();

        // Read from journalctl
        let journal_output = Command::new("journalctl")
            .args([
                "_COMM=kwin_wayland",
                "-o",
                "cat",
                "--since",
                &format!("@{}", start_time),
                "--no-pager",
            ])
            .output()
            .ok()?;

        let journal_logs = String::from_utf8_lossy(&journal_output.stdout);

        // Find our marked line and parse JSON
        for line in journal_logs.lines() {
            if line.starts_with(&format!("{}:", marker)) {
                let json_str = &line[marker.len() + 1..];
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let title = data
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let app_id = data
                        .get("app_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if title.is_empty() && app_id.is_empty() {
                        return None;
                    }

                    return Some(WindowInfo {
                        app_name: if !app_id.is_empty() {
                            app_id.clone()
                        } else {
                            "unknown".to_string()
                        },
                        window_title: title,
                        class_name: app_id,
                        focused: true,
                    });
                }
            }
        }

        None
    }

    fn get_active_window_via_x11_fallback(&self) -> Option<WindowInfo> {
        // Fallback: try X11 tools (may work on some KDE XWayland setups)
        Command::new("xdotool")
            .args(["getactivewindow", "getwindowname"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !title.is_empty() {
                        return Some(WindowInfo {
                            app_name: "unknown".to_string(),
                            window_title: title,
                            class_name: String::new(),
                            focused: true,
                        });
                    }
                }
                None
            })
    }
}

impl WindowManager for KDEWaylandManager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        // Try KWin script method (primary for KDE Wayland)
        self.get_active_window_via_kwin_script()
            .or_else(|| self.get_active_window_via_x11_fallback())
    }

    fn name(&self) -> &'static str {
        "kde-wayland"
    }
}
