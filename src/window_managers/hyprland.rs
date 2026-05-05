use serde::Deserialize;
use std::process::Command;

use super::{WindowInfo, WindowManager};

pub struct HyprlandManager;

impl HyprlandManager {
    pub fn new() -> Option<Self> {
        // Only create if HYPRLAND_INSTANCE_SIGNATURE is set (actually running Hyprland)
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            which::which("hyprctl").ok().map(|_| Self)
        } else {
            None
        }
    }
}

impl WindowManager for HyprlandManager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        let output = Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let json: HyprlandWindow = serde_json::from_slice(&output.stdout).ok()?;
        let app_class = json.app_class.unwrap_or_default();
        Some(WindowInfo {
            app_name: json.class.unwrap_or_else(|| app_class.clone()),
            window_title: json.title.unwrap_or_default(),
            class_name: app_class,
            focused: json.mapped.unwrap_or(true),
        })
    }

    fn name(&self) -> &'static str {
        "hyprland"
    }
}

#[derive(Debug, Deserialize)]
struct HyprlandWindow {
    #[serde(alias = "class")]
    class: Option<String>,
    #[serde(alias = "appClass")]
    app_class: Option<String>,
    #[serde(alias = "title")]
    title: Option<String>,
    #[serde(alias = "mapped")]
    mapped: Option<bool>,
}
