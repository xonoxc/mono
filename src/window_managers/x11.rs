use super::{WindowInfo, WindowManager};

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
