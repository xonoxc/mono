use log::{debug, info};
use std::sync::Arc;

use crate::models::Session;
use crate::storage::Storage;
use crate::window_manager::{WindowInfo, WindowManager};

/// Idle threshold in seconds — after this many seconds of no input,
/// we consider the user idle and pause the active session.
const IDLE_THRESHOLD_SECS: u64 = 120; // 2 minutes

/// Minimum session duration in seconds to persist.
/// Prevents writing thousands of sub-second sessions during rapid alt-tab.
const MIN_SESSION_DURATION_SECS: i64 = 2;

/// Event types that drive state transitions
#[derive(Debug)]
enum SessionEvent {
    WindowChanged(WindowInfo),
    IdleStarted,
    IdleEnded(WindowInfo),
    Shutdown,
}

/// Tracks the current state of the session manager
#[derive(Debug, PartialEq)]
enum TrackerState {
    Active,
    Idle,
    Stopped,
}

/// Session Manager: maintains the current in-memory session and
/// persists sessions only on state transitions (event-driven).
pub struct SessionManager {
    storage: Arc<Storage>,
    tracker: Box<dyn WindowManager>,
    current_session: Option<Session>,
    state: TrackerState,
    last_window: Option<WindowInfo>,
    was_idle: bool,
}

impl SessionManager {
    pub fn new(storage: Arc<Storage>, tracker: Box<dyn WindowManager>) -> Self {
        Self {
            storage,
            tracker,
            current_session: None,
            state: TrackerState::Active,
            last_window: None,
            was_idle: false,
        }
    }

    /// Called on each tick (~1s). Detects state transitions and
    /// triggers session open/close as needed.
    pub fn tick(&mut self) {
        if self.state == TrackerState::Stopped {
            return;
        }

        let idle_secs = self.tracker.get_idle_seconds();
        let current_window = self.tracker.get_active_window();

        // Determine the event
        let event = if idle_secs >= IDLE_THRESHOLD_SECS && !self.was_idle {
            // Just crossed idle threshold
            Some(SessionEvent::IdleStarted)
        } else if idle_secs < IDLE_THRESHOLD_SECS && self.was_idle {
            // Came back from idle
            current_window
                .as_ref()
                .map(|w| SessionEvent::IdleEnded(w.clone()))
        } else if !self.was_idle {
            // Check if window changed
            if let Some(ref window) = current_window {
                if self.has_window_changed(window) {
                    Some(SessionEvent::WindowChanged(window.clone()))
                } else {
                    None // No change, no event
                }
            } else {
                None
            }
        } else {
            None // Still idle, do nothing
        };

        // Process the event
        if let Some(event) = event {
            self.handle_event(event);
        }

        // Update idle tracking state
        self.was_idle = idle_secs >= IDLE_THRESHOLD_SECS;
    }

    fn has_window_changed(&self, new_window: &WindowInfo) -> bool {
        match &self.last_window {
            None => true,
            Some(last) => {
                last.app_name != new_window.app_name || last.window_title != new_window.window_title
            }
        }
    }

    fn handle_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::WindowChanged(window) => {
                debug!(
                    "Window changed → {} [{}]",
                    window.app_name, window.window_title
                );
                self.close_current_session();
                self.open_session(window);
            }

            SessionEvent::IdleStarted => {
                info!("Idle detected, pausing active session");
                self.close_current_session();
                // Open an idle session so we track idle duration
                let idle_session = Session::new_idle();
                self.storage.insert_session(&idle_session);
                self.current_session = Some(idle_session);
                self.state = TrackerState::Idle;
            }

            SessionEvent::IdleEnded(window) => {
                info!("User returned from idle");
                self.close_current_session();
                self.state = TrackerState::Active;
                self.open_session(window);
            }

            SessionEvent::Shutdown => {
                info!("Shutting down, closing current session");
                self.close_current_session();
                self.state = TrackerState::Stopped;
            }
        }
    }

    fn open_session(&mut self, window: WindowInfo) {
        let _session = Session::new(
            window.class_name.clone(), // Use WM_CLASS class name for consistency
            window.window_title.clone(),
        );

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
    }

    fn close_current_session(&mut self) {
        if let Some(mut session) = self.current_session.take() {
            session.close();

            // Only persist if session was long enough
            if session.duration_secs >= MIN_SESSION_DURATION_SECS || session.is_idle {
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

    /// Gracefully shutdown — close current session and mark stopped
    pub fn shutdown(&mut self) {
        self.handle_event(SessionEvent::Shutdown);
    }

    /// Check if the tracker is running
    pub fn is_running(&self) -> bool {
        self.state != TrackerState::Stopped
    }
}
