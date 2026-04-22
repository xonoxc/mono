use chrono::{Datelike, Local};
use rusqlite::{params, Connection};
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
    pub category: String,
}

pub struct TuiData {
    pub today_total: String,
    pub today_date: String,
    pub app_count: usize,
    pub weekly: Vec<DayData>,
    pub apps: Vec<AppData>,
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

        self.weekly.clear();

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

            self.weekly.push(DayData {
                label: date_label,
                date: date_str,
                seconds: secs,
                is_today,
            });
        }
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

    fn get_category(&self, app_name: &str) -> String {
        self.conn
            .query_row(
                "SELECT category FROM app_categories WHERE app_name = ?1",
                params![app_name],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "neutral".to_string())
    }

    pub fn load_day(&mut self, day_index: usize) {
        let date = self
            .weekly
            .get(day_index)
            .map(|day| day.date.clone())
            .unwrap_or_else(|| self.today_date.clone());

        self.apps = self.load_apps_for_date(&date);
        self.app_count = self.apps.len();
    }

    pub fn refresh_app_trend(&mut self, app_name: Option<&str>) {
        self.app_trend = self
            .weekly
            .iter()
            .map(|day| match app_name {
                Some(name) => self.app_seconds_for_date(&day.date, name) as u64,
                None => day.seconds.max(0) as u64,
            })
            .collect();
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
                    let category = self.get_category(&name);
                    Ok(AppData {
                        name,
                        seconds: secs,
                        category,
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
