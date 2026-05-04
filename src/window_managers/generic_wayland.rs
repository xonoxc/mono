use std::process::Command;

use super::{WindowInfo, WindowManager};

pub struct GenericWaylandManager;

impl GenericWaylandManager {
    pub fn new() -> Option<Self> {
        Some(Self)
    }
}

impl WindowManager for GenericWaylandManager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        get_active_window_wmctrl()
            .or_else(get_active_window_xdotool)
            .or_else(get_active_window_xprop)
    }

    fn name(&self) -> &'static str {
        "wayland-generic"
    }
}

fn get_active_window_wmctrl() -> Option<WindowInfo> {
    let output = Command::new("wmctrl").args(["-a", "-p"]).output().ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let line = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = line.trim().splitn(4, ' ').collect();
    if parts.len() >= 3 {
        Some(WindowInfo {
            app_name: parts[2].to_string(),
            window_title: String::new(),
            class_name: String::new(),
            focused: true,
        })
    } else {
        None
    }
}

fn get_active_window_xdotool() -> Option<WindowInfo> {
    let output = Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if title.is_empty() {
        return None;
    }

    Some(WindowInfo {
        app_name: String::new(),
        window_title: title,
        class_name: String::new(),
        focused: true,
    })
}

fn get_active_window_xprop() -> Option<WindowInfo> {
    let output = Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let window_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if window_id.is_empty() {
        return None;
    }

    let output = Command::new("xprop")
        .args(["-id", &window_id, "WM_CLASS"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let line = String::from_utf8_lossy(&output.stdout);
    if let Some(pos) = line.find("\"") {
        let start = pos + 1;
        if let Some(end) = line[start..].find("\"") {
            let app_name = line[start..start + end].to_string();
            return Some(WindowInfo {
                app_name: app_name.clone(),
                window_title: String::new(),
                class_name: app_name,
                focused: true,
            });
        }
    }

    None
}
