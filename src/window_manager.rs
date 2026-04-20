use log::debug;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub class_name: String,
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
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("WAYLAND_SOCKET").is_ok()
        {
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
    fn get_idle_seconds(&self) -> u64;
    fn name(&self) -> &'static str;
}

pub struct HyprlandManager;

impl HyprlandManager {
    pub fn new() -> Option<Self> {
        which::which("hyprctl").ok().map(|_| Self)
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
        })
    }

    fn get_idle_seconds(&self) -> u64 {
        let output = Command::new("hyprctl")
            .args(["misc", "zhypridle", "check"])
            .output()
            .ok();

        if let Some(output) = output {
            if output.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(sec) = val.get("seconds").and_then(|v| v.as_u64()) {
                        return sec;
                    }
                }
            }
        }

        get_idle_dbus().or_else(get_idle_loginctl).unwrap_or(0)
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

    fn get_idle_seconds(&self) -> u64 {
        get_idle_loginctl().unwrap_or(0)
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

    fn get_idle_seconds(&self) -> u64 {
        get_idle_dbus().or_else(get_idle_loginctl).unwrap_or(0)
    }

    fn name(&self) -> &'static str {
        "wayland-generic"
    }
}

fn get_active_window_wmctrl() -> Option<WindowInfo> {
    let output = Command::new("wmctrl")
        .args(["-a", "-p"])
        .output()
        .ok()?;

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
            });
        }
    }

    None
}

fn get_idle_dbus() -> Option<u64> {
    let output = Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.gnome.Mutter.IdleMonitor",
            "--print-reply",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "org.gnome.Mutter.IdleMonitor.GetIdletime",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(pos) = text.find("uint64 ") {
            let val = text[pos + 7..].trim().split_whitespace().next()?;
            return val.parse().ok();
        }
    }
    None
}

fn get_idle_loginctl() -> Option<u64> {
    let output = Command::new("loginctl")
        .args(["show-session", "-p", "IdleHintSince", "$(whoami)"])
        .output()
        .ok()?;

    if !output.status.success() {
        let output = Command::new("loginctl")
            .args(["seat-status", "-m"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("IdleHint=yes") || line.contains("active") {
                if let Some(ts) = line.split_whitespace().last() {
                    if let Ok(ts) = ts.parse::<i64>() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64;
                        return Some((now - ts).max(0) as u64);
                    }
                }
            }
        }
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    if let Some(pos) = text.find('=') {
        if let Ok(ts) = text[pos + 1..].trim().parse::<i64>() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            return Some((now - ts).max(0) as u64);
        }
    }
    None
}

pub struct X11Manager {
    tracker: crate::tracker::Tracker,
}

impl X11Manager {
    pub fn new() -> Option<Self> {
        crate::tracker::Tracker::new().map(|tracker| Self { tracker })
    }
}

impl WindowManager for X11Manager {
    fn get_active_window(&self) -> Option<WindowInfo> {
        self.tracker.get_active_window().map(|w| WindowInfo {
            app_name: w.app_name,
            window_title: w.window_title,
            class_name: w.class_name,
        })
    }

    fn get_idle_seconds(&self) -> u64 {
        self.tracker.get_idle_seconds()
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
        DisplayServer::Sway => {
            SwayManager::new().map(|m| Box::new(m) as Box<dyn WindowManager>)
        }
        DisplayServer::X11 => {
            X11Manager::new().map(|m| Box::new(m) as Box<dyn WindowManager>)
        }
        DisplayServer::Gnome | DisplayServer::KDE | DisplayServer::Wlroots => {
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