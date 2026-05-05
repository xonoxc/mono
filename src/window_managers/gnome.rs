use std::process::Command;

use super::{WindowInfo, WindowManager};

pub struct GnomeWaylandManager;

impl GnomeWaylandManager {
    pub fn new() -> Option<Self> {
        which::which("gdbus").ok().map(|_| Self)
    }

    fn get_active_window_via_eval(&self) -> Option<WindowInfo> {
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.gnome.Shell",
                "--object-path", "/org/gnome/Shell",
                "--method", "org.gnome.Shell.Eval",
                "var w = global.get_window_actors().find(a => a.meta_window && a.meta_window.has_focus()); \
                 if (w) { \
                   var mw = w.meta_window; \
                   JSON.stringify({ \
                     title: mw.get_title() || '', \
                     wm_class: mw.get_wm_class() || '', \
                     app_id: mw.get_app_id ? mw.get_app_id() : '' \
                   }); \
                 } else { ''; }",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let response = String::from_utf8_lossy(&output.stdout);
        // Response format: (true, '{"title":"...","wm_class":"...","app_id":"..."}')
        let json_str = response.trim();
        if !json_str.starts_with("(true, '") && !json_str.starts_with("(true, \"") {
            return None;
        }

        // Extract JSON from (true, '...')
        let start = json_str.find('\'').or_else(|| json_str.find('"'))? + 1;
        let end = json_str.rfind('\'').or_else(|| json_str.rfind('"'))?;
        if start >= end {
            return None;
        }

        let json_content = &json_str[start..end];
        let data: serde_json::Value = serde_json::from_str(json_content).ok()?;

        let title = data.get("title")?.as_str()?.to_string();
        let wm_class = data.get("wm_class").and_then(|v| v.as_str()).unwrap_or("");
        let app_id = data.get("app_id").and_then(|v| v.as_str()).unwrap_or("");

        let app_name = if !app_id.is_empty() {
            app_id.to_string()
        } else if !wm_class.is_empty() {
            wm_class.to_string()
        } else {
            "unknown".to_string()
        };

        Some(WindowInfo {
            app_name,
            window_title: title,
            class_name: wm_class.to_string(),
            focused: true,
        })
    }

    fn get_active_window_via_introspect(&self) -> Option<WindowInfo> {
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.gnome.Shell.Introspect",
                "--object-path", "/org/gnome/Shell/Introspect",
                "--method", "org.gnome.Shell.Introspect.GetWindows",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let response = String::from_utf8_lossy(&output.stdout);
        // Parse the complex variant output to find focused window
        // Format contains "has-focus: <true|false>" for each window
        for line in response.lines() {
            if line.contains("has-focus: true") {
                // Try to extract title and wm-class from surrounding context
                // This is a simplified parser for the gdbus output
                let title = extract_property(&response, "title");
                let wm_class = extract_property(&response, "wm-class");
                let app_id = extract_property(&response, "app-id");

                let app_name = if !app_id.is_empty() {
                    app_id
                } else if !wm_class.is_empty() {
                    wm_class.clone()
                } else {
                    "unknown".to_string()
                };

                return Some(WindowInfo {
                    app_name,
                    window_title: title,
                    class_name: wm_class,
                    focused: true,
                });
            }
        }
        None
    }
}

fn extract_property(response: &str, key: &str) -> String {
    let key_pattern = format!("{}:", key);
    if let Some(pos) = response.find(&key_pattern) {
        let start = pos + key_pattern.len();
        let rest = &response[start..];
        // Skip whitespace
        let rest = rest.trim_start();
        // Find the value - it's in quotes after the key
        if let Some(q) = rest.find('\'') {
            let start = q + 1;
            if let Some(end) = rest[start..].find('\'') {
                return rest[start..start + end].to_string();
            }
        }
    }
    String::new()
}

impl WindowManager for GnomeWaylandManager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        self.get_active_window_via_introspect()
            .or_else(|| self.get_active_window_via_eval())
    }

    fn name(&self) -> &'static str {
        "gnome-wayland"
    }
}
