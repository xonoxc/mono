use chrono::{Datelike, Local};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

#[derive(Clone)]
pub struct DayData {
    pub label: String,
    pub date: String,
    pub seconds: i64,
    pub is_today: bool,
}

#[derive(Clone)]
pub struct AppData {
    pub name: String,
    pub seconds: i64,
}

#[derive(Clone)]
pub struct DayStats {
    pub apps_used: usize,
    pub peak_hour: Option<String>,
    pub focus_time_seconds: i64,
}

pub struct TuiData {
    pub today_total: String,
    pub today_date: String,
    pub app_count: usize,
    pub weekly: Vec<DayData>,
    pub apps: Vec<AppData>,
    pub day_stats: Option<DayStats>,
    pub app_trend: Vec<u64>,
    pub live_app: Option<String>,
    pub live_seconds: i64,
    conn: Connection,
}

impl TuiData {
    pub fn new() -> Self {
        let conn = Connection::open(Self::db_path())
            .unwrap_or_else(|_| Connection::open_in_memory().expect("Failed to open database"));

        Self {
            today_total: "0m".to_string(),
            today_date: Local::now().format("%Y-%m-%d").to_string(),
            app_count: 0,
            weekly: Vec::new(),
            apps: Vec::new(),
            day_stats: None,
            app_trend: Vec::new(),
            live_app: None,
            live_seconds: 0,
            conn,
        }
    }

    fn db_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("screen-time-tracker")
            .join("screen_time.db")
    }

    pub fn refresh(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.today_date = today.clone();

        let total: i64 = self.conn
            .query_row(
                "SELECT COALESCE(SUM(duration_secs), 0) FROM sessions WHERE date = ?1 AND is_idle = 0",
                params![&today],
                |row| row.get(0),
            )
            .unwrap_or(0);

        self.today_total = format_time(total);
        self.refresh_weekly();
    }

    fn refresh_weekly(&mut self) {
        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let mut weekly = Vec::with_capacity(7);

        for i in (0..7).rev() {
            let date = today - chrono::Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();
            let date_label = days[date.weekday().num_days_from_sunday() as usize].to_string();
            let is_today = date_str == today_str;

            let secs: i64 = self.conn
                .query_row(
                    "SELECT COALESCE(SUM(duration_secs), 0) FROM sessions WHERE date = ?1 AND is_idle = 0",
                    params![&date_str],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            weekly.push(DayData {
                label: date_label,
                date: date_str,
                seconds: secs,
                is_today,
            });
        }

        let first_visible = weekly
            .iter()
            .position(|day| day.seconds > 0)
            .unwrap_or_else(|| weekly.len().saturating_sub(1));
        self.weekly = weekly.into_iter().skip(first_visible).collect();
    }

    pub fn refresh_live(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();

        if let Ok((app, secs)) = self.conn.query_row::<(String, i64), _, _>(
            "SELECT app_name, COALESCE(SUM(duration_secs), 0) as total
             FROM sessions
             WHERE date = ?1 AND end_time IS NULL AND is_idle = 0
             GROUP BY app_name
             ORDER BY total DESC
             LIMIT 1",
            params![&today],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            self.live_app = Some(app);
            self.live_seconds = secs;
        } else {
            self.live_app = None;
            self.live_seconds = 0;
        }
    }

    pub fn load_day(&mut self, day_index: usize) {
        let date = self
            .weekly
            .get(day_index)
            .map(|day| day.date.clone())
            .unwrap_or_else(|| self.today_date.clone());

        self.apps = self.load_apps_for_date(&date);
        self.day_stats = self.load_stats_for_date(&date);
        self.app_count = self
            .day_stats
            .as_ref()
            .map(|stats| stats.apps_used)
            .unwrap_or_else(|| self.apps.len());
    }

    pub fn refresh_app_trend(&mut self, app_name: Option<&str>) {
        self.app_trend = match app_name {
            Some(name) => self
                .weekly
                .iter()
                .map(|day| self.app_seconds_for_date(&day.date, name).max(0) as u64)
                .collect(),
            None => Vec::new(),
        };
    }

    pub fn weekly_average_seconds(&self) -> i64 {
        if self.weekly.is_empty() {
            0
        } else {
            self.weekly.iter().map(|day| day.seconds).sum::<i64>() / self.weekly.len() as i64
        }
    }

    fn load_apps_for_date(&self, date: &str) -> Vec<AppData> {
        match self.conn.prepare(
            "SELECT app_name, SUM(duration_secs) as total
             FROM sessions
             WHERE date = ?1 AND is_idle = 0
             GROUP BY app_name
             ORDER BY total DESC
             LIMIT 12",
        ) {
            Ok(mut stmt) => stmt
                .query_map(params![date], |row| {
                    let name: String = row.get(0)?;
                    let secs: i64 = row.get(1)?;
                    Ok(AppData {
                        name,
                        seconds: secs,
                    })
                })
                .map(|rows| rows.filter_map(|row| row.ok()).collect())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    fn app_seconds_for_date(&self, date: &str, app_name: &str) -> i64 {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(duration_secs), 0)
                 FROM sessions
                 WHERE date = ?1 AND app_name = ?2 AND is_idle = 0",
                params![date, app_name],
                |row| row.get(0),
            )
            .unwrap_or(0)
    }

    fn load_stats_for_date(&self, date: &str) -> Option<DayStats> {
        let apps_used: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT app_name)
                 FROM sessions
                 WHERE date = ?1 AND is_idle = 0 AND duration_secs > 0",
                params![date],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let peak_hour = self
            .conn
            .query_row(
                "SELECT CAST(strftime('%H', start_time) AS INTEGER) AS hour
                 FROM sessions
                 WHERE date = ?1 AND is_idle = 0 AND duration_secs > 0
                 GROUP BY hour
                 ORDER BY SUM(duration_secs) DESC, hour ASC
                 LIMIT 1",
                params![date],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .unwrap_or(None)
            .map(format_hour_label);

        let focus_time_seconds: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(duration_secs), 0)
                 FROM sessions
                 WHERE date = ?1 AND is_idle = 0 AND duration_secs >= 1500",
                params![date],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if apps_used == 0 && peak_hour.is_none() && focus_time_seconds <= 0 {
            None
        } else {
            Some(DayStats {
                apps_used: apps_used.max(0) as usize,
                peak_hour,
                focus_time_seconds,
            })
        }
    }
}

fn format_time(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{}h {}m", h, m)
    } else {
        format!("{}m", m)
    }
}

fn format_hour_label(hour_24: i64) -> String {
    let normalized = hour_24.rem_euclid(24);
    let meridiem = if normalized >= 12 { "PM" } else { "AM" };
    let hour_12 = match normalized % 12 {
        0 => 12,
        value => value,
    };
    format!("{hour_12} {meridiem}")
}
