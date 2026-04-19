use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

// ─── Core Session Model ──────────────────────────────────────────

/// A session represents a continuous period of using one application.
/// Sessions are opened on window change and closed when the active window changes,
/// idle is detected, or the daemon shuts down.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub app_name: String,
    pub window_title: String,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub duration_secs: i64,
    pub is_idle: bool,
    pub date: String,
}

impl Session {
    pub fn new(app_name: String, window_title: String) -> Self {
        let now = Local::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            app_name,
            window_title,
            start_time: now,
            end_time: None,
            duration_secs: 0,
            is_idle: false,
            date: now.format("%Y-%m-%d").to_string(),
        }
    }

    pub fn new_idle() -> Self {
        let now = Local::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            app_name: "__idle__".to_string(),
            window_title: "System Idle".to_string(),
            start_time: now,
            end_time: None,
            duration_secs: 0,
            is_idle: true,
            date: now.format("%Y-%m-%d").to_string(),
        }
    }

    pub fn close(&mut self) {
        let now = Local::now();
        self.end_time = Some(now);
        self.duration_secs = (now - self.start_time).num_seconds();
    }
}

// ─── Browser Session ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub id: String,
    pub url: String,
    pub title: String,
    pub domain: String,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
    pub duration_secs: i64,
    pub date: String,
}

impl BrowserSession {
    pub fn new(url: String, title: String, domain: String) -> Self {
        let now = Local::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            url,
            title,
            domain,
            start_time: now,
            end_time: None,
            duration_secs: 0,
            date: now.format("%Y-%m-%d").to_string(),
        }
    }
}

// ─── API Response Models ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodayUsage {
    pub date: String,
    pub total_seconds: i64,
    pub idle_seconds: i64,
    pub app_breakdown: Vec<AppBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBreakdown {
    pub app_name: String,
    pub seconds: i64,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyUsage {
    pub days: Vec<DaySummary>,
    pub total_seconds: i64,
    pub daily_average_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaySummary {
    pub date: String,
    pub total_seconds: i64,
    pub idle_seconds: i64,
    pub top_apps: Vec<AppBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub app_name: String,
    pub window_title: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_secs: i64,
    pub is_idle: bool,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsiteUsage {
    pub domain: String,
    pub total_seconds: i64,
    pub visits: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusScore {
    pub score: i64,
    pub productive_seconds: i64,
    pub distracting_seconds: i64,
    pub neutral_seconds: i64,
    pub total_active_seconds: i64,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppCategory {
    pub app_name: String,
    pub category: String,
    pub custom_name: Option<String>,
}

// ─── Browser Extension Payload ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTabEvent {
    pub url: String,
    pub title: String,
    pub domain: String,
    pub event_type: String, // "focus" | "blur" | "navigate"
}

// ─── API wrapper ─────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { ok: true, data }
    }
}
