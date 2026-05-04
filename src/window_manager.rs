use log::debug;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub class_name: String,
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DisplayServer {
    #[default]
    Unknown,
    X11,
    Hyprland,
    Sway,
    Gnome,
    KDE,
    Wlroots,
}

impl DisplayServer {
    pub fn detect() -> Self {
        if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("WAYLAND_SOCKET").is_ok() {
            if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
                return DisplayServer::Hyprland;
            }
            if std::env::var("SWAYSOCK").is_ok() {
                return DisplayServer::Sway;
            }
            if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
                return DisplayServer::Gnome;
            }
            if std::env::var("KDE_FULL_SESSION").is_ok() {
                return DisplayServer::KDE;
            }
            if std::env::var("XDG_CURRENT_DESKTOP")
                .map(|d| d.to_lowercase().contains("gnome"))
                .unwrap_or(false)
            {
                return DisplayServer::Gnome;
            }
            if std::env::var("XDG_CURRENT_DESKTOP")
                .map(|d| d.to_lowercase().contains("kde"))
                .unwrap_or(false)
            {
                return DisplayServer::KDE;
            }
            return DisplayServer::Wlroots;
        }

        if std::env::var("DISPLAY").is_ok() {
            return DisplayServer::X11;
        }

        DisplayServer::Unknown
    }
}

#[allow(async_fn_in_trait)]
pub trait WindowManager: Send + Sync {
    fn get_active_window(&self) -> Option<WindowInfo>;
    fn name(&self) -> &'static str;
}

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

pub struct SwayManager;

impl SwayManager {
    pub fn new() -> Option<Self> {
        which::which("swaymsg").ok().map(|_| Self)
    }
}

impl WindowManager for SwayManager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        let output = Command::new("swaymsg")
            .args(["-t", "get_tree", "-r"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let tree: SwayTree = serde_json::from_slice(&output.stdout).ok()?;

        fn find_focused(node: &SwayNode) -> Option<WindowInfo> {
            if node.focused {
                return Some(WindowInfo {
                    app_name: node
                        .app_id
                        .clone()
                        .or_else(|| node.name.clone())
                        .unwrap_or_default(),
                    window_title: node.name.clone().unwrap_or_default(),
                    class_name: node.app_id.clone().unwrap_or_default(),
                    focused: true,
                });
            }
            for child in &node.nodes {
                if let Some(info) = find_focused(child) {
                    return Some(info);
                }
            }
            for child in &node.floating_nodes {
                if let Some(info) = find_focused(child) {
                    return Some(info);
                }
            }
            None
        }

        find_focused(&tree.root)
    }

    fn name(&self) -> &'static str {
        "sway"
    }
}

#[derive(Debug, Deserialize)]
struct SwayTree {
    root: SwayNode,
}

#[derive(Debug, Deserialize)]
struct SwayNode {
    name: Option<String>,
    app_id: Option<String>,
    focused: bool,
    nodes: Vec<SwayNode>,
    #[serde(alias = "floating_nodes")]
    floating_nodes: Vec<SwayNode>,
}

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

pub struct X11Manager;

impl X11Manager {
    pub fn new() -> Option<Self> {
        // X11 support removed - use Wayland or other display servers
        None
    }
}

impl WindowManager for X11Manager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        None
    }

    fn name(&self) -> &'static str {
        "x11"
    }
}

pub fn create_manager() -> Option<Box<dyn WindowManager>> {
    let server = DisplayServer::detect();
    debug!("Detected display server: {:?}", server);

    let manager: Option<Box<dyn WindowManager>> = match server {
        DisplayServer::Hyprland => {
            HyprlandManager::new().map(|m| Box::new(m) as Box<dyn WindowManager>)
        }
        DisplayServer::Sway => SwayManager::new().map(|m| Box::new(m) as Box<dyn WindowManager>),
        DisplayServer::X11 => X11Manager::new().map(|m| Box::new(m) as Box<dyn WindowManager>),
        DisplayServer::Gnome => {
            GnomeWaylandManager::new()
                .map(|m| Box::new(m) as Box<dyn WindowManager>)
                .or_else(|| {
                    GenericWaylandManager::new()
                        .map(|m| Box::new(m) as Box<dyn WindowManager>)
                })
        }
        DisplayServer::KDE => {
            KDEWaylandManager::new()
                .map(|m| Box::new(m) as Box<dyn WindowManager>)
                .or_else(|| {
                    GenericWaylandManager::new()
                        .map(|m| Box::new(m) as Box<dyn WindowManager>)
                })
        }
        DisplayServer::Wlroots => {
            if let Some(m) = HyprlandManager::new() {
                return Some(Box::new(m) as Box<dyn WindowManager>);
            }
            if let Some(m) = SwayManager::new() {
                return Some(Box::new(m) as Box<dyn WindowManager>);
            }
            GenericWaylandManager::new().map(|m| Box::new(m) as Box<dyn WindowManager>)
        }
        DisplayServer::Unknown => {
            if let Some(m) = X11Manager::new() {
                return Some(Box::new(m) as Box<dyn WindowManager>);
            }
            if let Some(m) = HyprlandManager::new() {
                return Some(Box::new(m) as Box<dyn WindowManager>);
            }
            if let Some(m) = SwayManager::new() {
                return Some(Box::new(m) as Box<dyn WindowManager>);
            }
            GenericWaylandManager::new().map(|m| Box::new(m) as Box<dyn WindowManager>)
        }
    };

    manager
}
