use log::error;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_ulong};
use std::ptr;

/// Information about the currently active window
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub app_name: String,     // WM_CLASS instance
    pub window_title: String, // _NET_WM_NAME or WM_NAME
    pub class_name: String,   // WM_CLASS class
}

/// X11-based window tracker and idle detector
pub struct Tracker {
    display: *mut x11::xlib::Display,
}

// We manually manage the display pointer, safe to send across threads
unsafe impl Send for Tracker {}
unsafe impl Sync for Tracker {}

impl Tracker {
    /// Opens a connection to the X11 display
    pub fn new() -> Option<Self> {
        unsafe {
            let display = x11::xlib::XOpenDisplay(ptr::null());
            if display.is_null() {
                error!("Failed to open X11 display. Are you running X11?");
                return None;
            }
            Some(Self { display })
        }
    }

    /// Get the currently active/focused window
    pub fn get_active_window(&self) -> Option<WindowInfo> {
        unsafe {
            let root = x11::xlib::XDefaultRootWindow(self.display);

            // Get _NET_ACTIVE_WINDOW property
            let atom = x11::xlib::XInternAtom(
                self.display,
                b"_NET_ACTIVE_WINDOW\0".as_ptr() as *const c_char,
                x11::xlib::False,
            );

            let mut actual_type: c_ulong = 0;
            let mut actual_format: c_int = 0;
            let mut nitems: c_ulong = 0;
            let mut bytes_after: c_ulong = 0;
            let mut prop: *mut u8 = ptr::null_mut();

            let status = x11::xlib::XGetWindowProperty(
                self.display,
                root,
                atom,
                0,
                1,
                x11::xlib::False,
                x11::xlib::XA_WINDOW,
                &mut actual_type,
                &mut actual_format,
                &mut nitems,
                &mut bytes_after,
                &mut prop,
            );

            if status != 0 || nitems == 0 || prop.is_null() {
                if !prop.is_null() {
                    x11::xlib::XFree(prop as *mut _);
                }
                return None;
            }

            let window = *(prop as *const c_ulong);
            x11::xlib::XFree(prop as *mut _);

            if window == 0 {
                return None;
            }

            let title = self.get_window_title(window);
            let (app_name, class_name) = self.get_window_class(window);

            Some(WindowInfo {
                app_name,
                window_title: title,
                class_name,
            })
        }
    }

    /// Get the window title (_NET_WM_NAME or WM_NAME)
    fn get_window_title(&self, window: c_ulong) -> String {
        unsafe {
            // Try _NET_WM_NAME first (UTF-8)
            let utf8_atom = x11::xlib::XInternAtom(
                self.display,
                b"_NET_WM_NAME\0".as_ptr() as *const c_char,
                x11::xlib::False,
            );
            let utf8_type = x11::xlib::XInternAtom(
                self.display,
                b"UTF8_STRING\0".as_ptr() as *const c_char,
                x11::xlib::False,
            );

            let mut actual_type: c_ulong = 0;
            let mut actual_format: c_int = 0;
            let mut nitems: c_ulong = 0;
            let mut bytes_after: c_ulong = 0;
            let mut prop: *mut u8 = ptr::null_mut();

            let status = x11::xlib::XGetWindowProperty(
                self.display,
                window,
                utf8_atom,
                0,
                1024,
                x11::xlib::False,
                utf8_type,
                &mut actual_type,
                &mut actual_format,
                &mut nitems,
                &mut bytes_after,
                &mut prop,
            );

            if status == 0 && nitems > 0 && !prop.is_null() {
                let title = String::from_utf8_lossy(
                    std::slice::from_raw_parts(prop, nitems as usize)
                ).to_string();
                x11::xlib::XFree(prop as *mut _);
                return title;
            }

            if !prop.is_null() {
                x11::xlib::XFree(prop as *mut _);
            }

            // Fallback to WM_NAME
            let mut name_ptr: *mut c_char = ptr::null_mut();
            if x11::xlib::XFetchName(self.display, window, &mut name_ptr) != 0
                && !name_ptr.is_null()
            {
                let title = CStr::from_ptr(name_ptr).to_string_lossy().to_string();
                x11::xlib::XFree(name_ptr as *mut _);
                return title;
            }

            String::new()
        }
    }

    /// Get WM_CLASS (instance name, class name)
    fn get_window_class(&self, window: c_ulong) -> (String, String) {
        unsafe {
            let mut class_hint = x11::xlib::XClassHint {
                res_name: ptr::null_mut(),
                res_class: ptr::null_mut(),
            };

            if x11::xlib::XGetClassHint(self.display, window, &mut class_hint) != 0 {
                let instance = if !class_hint.res_name.is_null() {
                    let s = CStr::from_ptr(class_hint.res_name).to_string_lossy().to_string();
                    x11::xlib::XFree(class_hint.res_name as *mut _);
                    s
                } else {
                    String::new()
                };

                let class = if !class_hint.res_class.is_null() {
                    let s = CStr::from_ptr(class_hint.res_class).to_string_lossy().to_string();
                    x11::xlib::XFree(class_hint.res_class as *mut _);
                    s
                } else {
                    String::new()
                };

                (instance, class)
            } else {
                (String::new(), String::new())
            }
        }
    }

    /// Get idle time in seconds using XScreenSaver extension
    pub fn get_idle_seconds(&self) -> u64 {
        unsafe {
            // Check if XScreenSaver extension is available
            let mut event_base: c_int = 0;
            let mut error_base: c_int = 0;
            if x11::xss::XScreenSaverQueryExtension(
                self.display,
                &mut event_base,
                &mut error_base,
            ) == 0
            {
                return 0;
            }

            let info = x11::xss::XScreenSaverAllocInfo();
            if info.is_null() {
                return 0;
            }

            let root = x11::xlib::XDefaultRootWindow(self.display);
            x11::xss::XScreenSaverQueryInfo(self.display, root, info);
            let idle_ms = (*info).idle;
            x11::xlib::XFree(info as *mut _);

            idle_ms as u64 / 1000
        }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe {
            if !self.display.is_null() {
                x11::xlib::XCloseDisplay(self.display);
            }
        }
    }
}
