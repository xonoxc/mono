mod generic_wayland;
mod gnome;
mod hyprland;
mod kde;
mod sway;
mod x11;

use log::debug;

pub use generic_wayland::GenericWaylandManager;
pub use gnome::GnomeWaylandManager;
pub use hyprland::HyprlandManager;
pub use kde::KDEWaylandManager;
pub use sway::SwayManager;
pub use x11::X11Manager;

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
