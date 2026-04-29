use chrono::Local;
use log::{debug, info};
use std::sync::Arc;

use crate::models::Session;
use crate::storage::Storage;
use crate::window_manager::{WindowInfo, WindowManager};

/// Minimum session duration in seconds to persist.
/// Prevents writing thousands of sub-second sessions during rapid alt-tab.
const MIN_SESSION_DURATION_SECS: i64 = 2;

/// Unfocused threshold in seconds — after this many seconds of unfocused window,
/// we close the session and subtract the unfocused time.
const UNFOCUSED_THRESHOLD_SECS: u64 = 20;

/// Session Manager: maintains the current in-memory session and
/// persists sessions only on state transitions (event-driven).
/// Tracks unfocused time and subtracts it from session duration.
pub struct SessionManager {
    storage: Arc<Storage>,
    tracker: Box<dyn WindowManager>,
    current_session: Option<Session>,
    state: TrackerState,
    last_window: Option<WindowInfo>,
    unfocus_start: Option<chrono::DateTime<Local>>,
    total_unfocus_secs: u64,
}

#[derive(Debug, PartialEq)]
enum TrackerState {
    Active,
    Stopped,
}

impl SessionManager {
    pub fn new(storage: Arc<Storage>, tracker: Box<dyn WindowManager>) -> Self {
        storage.close_all_open_sessions();
        Self {
            storage,
            tracker,
            current_session: None,
            state: TrackerState::Active,
            last_window: None,
            unfocus_start: None,
            total_unfocus_secs: 0,
        }
    }

    /// Called on each tick (~1s). Detects state transitions and
    /// triggers session open/close as needed.
    pub fn tick(&mut self) {
        if self.state == TrackerState::Stopped {
            return;
        }

        let current_window = self.tracker.get_active_window();
        let now = Local::now();

        match (&self.current_session, &current_window) {
            // Case 1: No current session → start new one if window is focused
            (None, Some(window)) if window.focused => {
                debug!(
                    "Opening new session → {} [{}]",
                    window.app_name, window.window_title
                );
                self.open_session(window.clone());
            }

            // Case 2: Have session, window changed → close old, open new if focused
            (Some(_), Some(window)) if self.has_window_changed(window) => {
                debug!(
                    "Window changed → {} [{}]",
                    window.app_name, window.window_title
                );
                self.close_current_session();
                if window.focused {
                    self.open_session(window.clone());
                }
            }

            // Case 3: Window unfocused (focused=false)
            (Some(_), Some(window)) if !window.focused => {
                if self.unfocus_start.is_none() {
                    self.unfocus_start = Some(now);
                    debug!("Window unfocused, starting unfocus timer");
                }
                // Check if unfocused > 20 sec
                if let Some(start) = self.unfocus_start {
                    let unfocused = (now - start).num_seconds() as u64;
                    if unfocused >= UNFOCUSED_THRESHOLD_SECS {
                        info!("Unfocused for {} sec, closing session", unfocused);
                        self.total_unfocus_secs += unfocused;
                        self.close_current_session_with_adjustment();
                    }
                }
            }

            // Case 4: Window refocused within 20 sec
            (Some(_), Some(window)) if window.focused => {
                if let Some(start) = self.unfocus_start {
                    let unfocused = (now - start).num_seconds() as u64;
                    if unfocused < UNFOCUSED_THRESHOLD_SECS {
                        // Continue session, track unfocused time
                        self.total_unfocus_secs += unfocused;
                        debug!("Window refocused after {} sec, continuing session", unfocused);
                    }
                    self.unfocus_start = None;
                }
            }

            // Case 5: No window active (None returned)
            (Some(_), None) => {
                info!("No active window, closing session");
                self.close_current_session();
            }

            _ => {}
        }
    }

    fn has_window_changed(&self, new_window: &WindowInfo) -> bool {
        match &self.last_window {
            None => true,
            Some(last) => {
                last.app_name != new_window.app_name || last.window_title != new_window.window_title
            }
        }
    }

    fn open_session(&mut self, window: WindowInfo) {
        // Use class_name if available, fall back to app_name
        let app = if !window.class_name.is_empty() {
            &window.class_name
        } else {
            &window.app_name
        };

        let session = Session::new(app.to_string(), window.window_title.clone());

        self.storage.insert_session(&session);
        self.current_session = Some(session);
        self.last_window = Some(window);
        self.unfocus_start = None;
    }

    fn close_current_session(&mut self) {
        if let Some(mut session) = self.current_session.take() {
            session.close();

            // Only persist if session was long enough
            if session.duration_secs >= MIN_SESSION_DURATION_SECS {
                self.storage.close_session(
                    &session.id,
                    session.end_time.unwrap(),
                    session.duration_secs,
                );
                debug!(
                    "Closed session: {} ({} secs)",
                    session.app_name, session.duration_secs
                );
            } else {
                self.storage.delete_session(&session.id);
                debug!(
                    "Discarding short session: {} ({} secs)",
                    session.app_name, session.duration_secs
                );
            }
        }
    }

    fn close_current_session_with_adjustment(&mut self) {
        if let Some(mut session) = self.current_session.take() {
            session.close();

            // Subtract unfocused time from duration
            let adjusted_duration = (session.duration_secs - self.total_unfocus_secs as i64)
                .max(0);

            if adjusted_duration >= MIN_SESSION_DURATION_SECS {
                self.storage.close_session(
                    &session.id,
                    session.end_time.unwrap(),
                    adjusted_duration,
                );
                debug!(
                    "Closed session with adjustment: {} ({} secs, {} unfocused)",
                    session.app_name, adjusted_duration, self.total_unfocus_secs
                );
            } else {
                self.storage.delete_session(&session.id);
                debug!(
                    "Discarding session after adjustment: {} ({} secs adjusted from {})",
                    session.app_name, adjusted_duration, session.duration_secs
                );
            }

            self.total_unfocus_secs = 0;
            self.unfocus_start = None;
        }
    }

    /// Gracefully shutdown — close current session and mark stopped
    pub fn shutdown(&mut self) {
        if self.current_session.is_some() {
            self.close_current_session_with_adjustment();
        }
        self.state = TrackerState::Stopped;
        info!("Session manager stopped");
    }

    /// Check if the tracker is running
    pub fn is_running(&self) -> bool {
        self.state != TrackerState::Stopped
    }
}
