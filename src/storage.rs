use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone};
use log::{error, info, warn};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Arc;

use crate::models::*;

/// SQLite storage layer for screen time data.
/// Uses WAL mode for performance and crash resilience.
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    pub fn new() -> Self {
        let db_path = Self::db_path();
        std::fs::create_dir_all(db_path.parent().unwrap()).ok();

        let conn = Connection::open(&db_path)
            .unwrap_or_else(|e| panic!("Failed to open database at {:?}: {}", db_path, e));

        // Performance tuning
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -2000;
             PRAGMA temp_store = MEMORY;
             PRAGMA busy_timeout = 5000;",
        )
        .expect("Failed to set pragmas");

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.run_migrations();
        sanitize_session_records(&storage.conn.lock());

        storage
    }

    fn db_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("screen-time-tracker")
            .join("screen_time.db")
    }

    fn run_migrations(&self) {
        let conn = self.conn.lock();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL DEFAULT '',
                start_time TEXT NOT NULL,
                end_time TEXT,
                duration_secs INTEGER DEFAULT 0,
                is_idle INTEGER DEFAULT 0,
                date TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS app_categories (
                app_name TEXT PRIMARY KEY,
                category TEXT NOT NULL DEFAULT 'neutral',
                custom_name TEXT,
                icon TEXT
            );

            CREATE TABLE IF NOT EXISTS browser_sessions (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                domain TEXT NOT NULL DEFAULT '',
                start_time TEXT NOT NULL,
                end_time TEXT,
                duration_secs INTEGER DEFAULT 0,
                date TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- Indexes for efficient querying
            CREATE INDEX IF NOT EXISTS idx_sessions_date ON sessions(date);
            CREATE INDEX IF NOT EXISTS idx_sessions_app_date ON sessions(app_name, date);
            CREATE INDEX IF NOT EXISTS idx_sessions_start ON sessions(start_time);
            CREATE INDEX IF NOT EXISTS idx_sessions_end ON sessions(end_time);
            CREATE INDEX IF NOT EXISTS idx_browser_date ON browser_sessions(date);
            CREATE INDEX IF NOT EXISTS idx_browser_domain ON browser_sessions(domain, date);",
        )
        .expect("Failed to run migrations");

        // Seed default categories
        Self::seed_categories(&conn);

        info!("Database migrations complete");
    }

    fn seed_categories(conn: &Connection) {
        let defaults = vec![
            ("code", "productive", "VS Code"),
            ("Code", "productive", "VS Code"),
            ("jetbrains", "productive", "JetBrains IDE"),
            ("vim", "productive", "Vim"),
            ("nvim", "productive", "Neovim"),
            ("Alacritty", "productive", "Alacritty"),
            ("kitty", "productive", "Kitty"),
            ("foot", "productive", "Foot"),
            ("gnome-terminal", "productive", "Terminal"),
            ("konsole", "productive", "Konsole"),
            ("wezterm", "productive", "WezTerm"),
            ("Slack", "neutral", "Slack"),
            ("discord", "distracting", "Discord"),
            ("Discord", "distracting", "Discord"),
            ("telegram", "neutral", "Telegram"),
            ("firefox", "neutral", "Firefox"),
            ("Firefox", "neutral", "Firefox"),
            ("chromium", "neutral", "Chromium"),
            ("google-chrome", "neutral", "Chrome"),
            ("Google-chrome", "neutral", "Chrome"),
            ("Navigator", "neutral", "Firefox"),
            ("Spotify", "distracting", "Spotify"),
            ("steam", "distracting", "Steam"),
            ("Steam", "distracting", "Steam"),
            ("vlc", "distracting", "VLC"),
            ("mpv", "distracting", "mpv"),
            ("thunar", "neutral", "Thunar"),
            ("nautilus", "neutral", "Files"),
            ("Nautilus", "neutral", "Files"),
            ("obs", "productive", "OBS Studio"),
            ("Gimp", "productive", "GIMP"),
            ("Inkscape", "productive", "Inkscape"),
            ("libreoffice", "productive", "LibreOffice"),
        ];

        let mut stmt = conn.prepare(
            "INSERT OR IGNORE INTO app_categories (app_name, category, custom_name) VALUES (?1, ?2, ?3)"
        ).unwrap();

        for (app, cat, name) in defaults {
            stmt.execute(params![app, cat, name]).ok();
        }
    }

    // ─── Session CRUD ────────────────────────────────────────────

    pub fn insert_session(&self, session: &Session) {
        let conn = self.conn.lock();
        close_open_sessions_before(&conn, &session.start_time);
        conn.execute(
            "INSERT INTO sessions (id, app_name, window_title, start_time, end_time, duration_secs, is_idle, date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                session.id,
                session.app_name,
                session.window_title,
                session.start_time.to_rfc3339(),
                session.end_time.map(|t| t.to_rfc3339()),
                session.duration_secs,
                session.date,
            ],
        ).unwrap_or_else(|e| {
            error!("Failed to insert session: {}", e);
            0
        });
    }

    pub fn close_session(&self, session_id: &str, end_time: DateTime<Local>, duration: i64) {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sessions SET end_time = ?1, duration_secs = ?2 WHERE id = ?3",
            params![end_time.to_rfc3339(), duration, session_id],
        )
        .unwrap_or_else(|e| {
            error!("Failed to close session {}: {}", session_id, e);
            0
        });
    }

    pub fn close_all_open_sessions(&self) {
        let conn = self.conn.lock();
        let result = conn.execute(
            "DELETE FROM sessions WHERE end_time IS NULL",
            params![],
        );
        if let Ok(count) = result {
            if count > 0 {
                warn!("Deleted {} orphan sessions from previous run", count);
            }
        }
    }

    pub fn delete_session(&self, session_id: &str) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])
            .unwrap_or_else(|e| {
                error!("Failed to delete session {}: {}", session_id, e);
                0
            });
    }

    // ─── Browser Sessions ────────────────────────────────────────

    pub fn insert_browser_session(&self, session: &BrowserSession) {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO sessions (id, app_name, window_title, start_time, end_time, duration_secs, is_idle, date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                session.id,
                format!("browser:{}", session.domain),
                session.title,
                session.start_time.to_rfc3339(),
                session.end_time.map(|t| t.to_rfc3339()),
                session.duration_secs,
                session.date,
            ],
        ).ok();

        conn.execute(
            "INSERT INTO browser_sessions (id, url, title, domain, start_time, end_time, duration_secs, date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.id,
                session.url,
                session.title,
                session.domain,
                session.start_time.to_rfc3339(),
                session.end_time.map(|t| t.to_rfc3339()),
                session.duration_secs,
                session.date,
            ],
        ).unwrap_or_else(|e| {
            error!("Failed to insert browser session: {}", e);
            0
        });
    }

    // ─── Query Methods ───────────────────────────────────────────

    pub fn get_today_usage(&self) -> TodayUsage {
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.get_day_usage(&today)
    }

    pub fn get_day_usage(&self, date: &str) -> TodayUsage {
        let conn = self.conn.lock();

        let total_seconds: i64 = conn.query_row(
            "SELECT COALESCE(SUM(duration_secs), 0) FROM sessions WHERE date = ?1",
            params![date],
            |row| row.get(0),
        ).unwrap_or(0);

        let mut stmt = conn
            .prepare(
                "SELECT app_name, SUM(duration_secs) as total
             FROM sessions
             WHERE date = ?1
             GROUP BY app_name
             ORDER BY total DESC",
            )
            .unwrap();

        let app_breakdown: Vec<AppBreakdown> = stmt
            .query_map(params![date], |row| {
                let app_name: String = row.get(0)?;
                let seconds: i64 = row.get(1)?;
                Ok((app_name, seconds))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .map(|(app_name, seconds)| {
                let category = self.get_category_unlocked(&conn, &app_name);
                AppBreakdown {
                    app_name,
                    seconds,
                    category,
                }
            })
            .collect();

        TodayUsage {
            date: date.to_string(),
            total_seconds,
            idle_seconds: 0,
            app_breakdown,
        }
    }

    pub fn get_weekly_usage(&self) -> WeeklyUsage {
        let today = Local::now().date_naive();
        let mut days = Vec::new();

        for i in (0..7).rev() {
            let date = today - chrono::Duration::days(i);
            let date_str = date.format("%Y-%m-%d").to_string();
            let usage = self.get_day_usage(&date_str);
            days.push(DaySummary {
                date: date_str,
                total_seconds: usage.total_seconds,
                idle_seconds: 0,
                top_apps: usage.app_breakdown.iter().take(5).cloned().collect(),
            });
        }

        let total: i64 = days.iter().map(|d| d.total_seconds).sum();
        let avg = if days.is_empty() {
            0
        } else {
            total / days.len() as i64
        };

        WeeklyUsage {
            days,
            total_seconds: total,
            daily_average_seconds: avg,
        }
    }

    pub fn get_app_breakdown(&self, date: Option<&str>) -> Vec<AppBreakdown> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let target_date = date.unwrap_or(&today);

        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT app_name, SUM(duration_secs) as total
             FROM sessions
             WHERE date = ?1
             GROUP BY app_name
             ORDER BY total DESC",
            )
            .unwrap();

        stmt.query_map(params![target_date], |row| {
            let app_name: String = row.get(0)?;
            let seconds: i64 = row.get(1)?;
            Ok((app_name, seconds))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|(app_name, seconds)| {
            let category = self.get_category_unlocked(&conn, &app_name);
            AppBreakdown {
                app_name,
                seconds,
                category,
            }
        })
        .collect()
    }

    pub fn get_session_history(&self, date: Option<&str>, limit: i64) -> Vec<SessionRecord> {
        let conn = self.conn.lock();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let target = date.unwrap_or(&today);

        let mut stmt = conn.prepare(
            "SELECT id, app_name, window_title, start_time, end_time, duration_secs, 0, date
             FROM sessions
             WHERE date = ?1
             ORDER BY start_time DESC
             LIMIT ?2"
        ).unwrap();

        stmt.query_map(params![target, limit], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                app_name: row.get(1)?,
                window_title: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                duration_secs: row.get(5)?,
                is_idle: false,
                date: row.get(7)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn get_website_usage(&self, date: Option<&str>) -> Vec<WebsiteUsage> {
        let conn = self.conn.lock();
        let today = Local::now().format("%Y-%m-%d").to_string();
        let target = date.unwrap_or(&today);

        let mut stmt = conn
            .prepare(
                "SELECT domain, SUM(duration_secs) as total, COUNT(*) as visits
             FROM browser_sessions
             WHERE date = ?1
             GROUP BY domain
             ORDER BY total DESC",
            )
            .unwrap();

        stmt.query_map(params![target], |row| {
            Ok(WebsiteUsage {
                domain: row.get(0)?,
                total_seconds: row.get(1)?,
                visits: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn get_focus_score(&self, date: Option<&str>) -> FocusScore {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let target = date.unwrap_or(&today);
        let usage = self.get_day_usage(target);

        let mut productive_secs: i64 = 0;
        let mut distracting_secs: i64 = 0;
        let mut neutral_secs: i64 = 0;

        for app in &usage.app_breakdown {
            match app.category.as_str() {
                "productive" => productive_secs += app.seconds,
                "distracting" => distracting_secs += app.seconds,
                _ => neutral_secs += app.seconds,
            }
        }

        let total_active = productive_secs + distracting_secs + neutral_secs;
        let score = if total_active > 0 {
            let ratio = productive_secs as f64 / total_active as f64;
            (ratio * 100.0).round() as i64
        } else {
            0
        };

        FocusScore {
            score,
            productive_seconds: productive_secs,
            distracting_seconds: distracting_secs,
            neutral_seconds: neutral_secs,
            total_active_seconds: total_active,
            date: target.to_string(),
        }
    }

    // ─── Category Management ─────────────────────────────────────

    pub fn get_category(&self, app_name: &str) -> String {
        let conn = self.conn.lock();
        self.get_category_unlocked(&conn, app_name)
    }

    fn get_category_unlocked(&self, conn: &Connection, app_name: &str) -> String {
        // Try exact match first
        if let Ok(cat) = conn.query_row(
            "SELECT category FROM app_categories WHERE app_name = ?1",
            params![app_name],
            |row| row.get::<_, String>(0),
        ) {
            return cat;
        }

        // Try case-insensitive partial match
        if let Ok(cat) = conn.query_row(
            "SELECT category FROM app_categories WHERE ?1 LIKE '%' || app_name || '%'",
            params![app_name.to_lowercase()],
            |row| row.get::<_, String>(0),
        ) {
            return cat;
        }

        "neutral".to_string()
    }

    pub fn set_category(&self, app_name: &str, category: &str) {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO app_categories (app_name, category) VALUES (?1, ?2)",
            params![app_name, category],
        )
        .ok();
    }

    pub fn get_all_categories(&self) -> Vec<AppCategory> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT app_name, category, custom_name FROM app_categories ORDER BY app_name")
            .unwrap();

        stmt.query_map([], |row| {
            Ok(AppCategory {
                app_name: row.get(0)?,
                category: row.get(1)?,
                custom_name: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }
}

pub fn sanitize_session_records(conn: &Connection) {
    let rows = match conn.prepare(
        "SELECT id, start_time, end_time, date
         FROM sessions
         WHERE end_time IS NULL
            OR duration_secs < 0
            OR duration_secs > 86400
            OR substr(start_time, 1, 10) != date
            OR (end_time IS NOT NULL AND substr(end_time, 1, 10) != date)",
    ) {
        Ok(mut stmt) => stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect::<Vec<_>>())
            .unwrap_or_default(),
        Err(err) => {
            warn!("Failed to scan sessions for repair: {}", err);
            return;
        }
    };

    if !rows.is_empty() {
        let now = Local::now().fixed_offset();
        warn!("Repairing {} suspicious session rows", rows.len());

        for (id, start_text, end_text, date_text) in rows {
            let Some(start_time) = parse_rfc3339(&start_text) else {
                continue;
            };
            let Some(day_end) = end_of_day(&date_text, start_time.offset()) else {
                continue;
            };

            let candidate_end = end_text.as_deref().and_then(parse_rfc3339).unwrap_or(now);
            let repaired_end = candidate_end.min(day_end).max(start_time);
            let duration_secs = (repaired_end - start_time).num_seconds().max(0);

            if let Err(err) = conn.execute(
                "UPDATE sessions
                 SET end_time = ?1, duration_secs = ?2
                 WHERE id = ?3",
                params![repaired_end.to_rfc3339(), duration_secs, id],
            ) {
                warn!("Failed to repair session {}: {}", id, err);
            }
        }
    }

    normalize_session_timeline(conn);
}

fn close_open_sessions_before(conn: &Connection, next_start: &DateTime<Local>) {
    let next_start = next_start.to_rfc3339();
    if let Err(err) = conn.execute(
        "UPDATE sessions
         SET end_time = ?1,
             duration_secs = MAX(
                 0,
                 CAST((julianday(?1) - julianday(start_time)) * 86400 AS INTEGER)
             )
         WHERE end_time IS NULL",
        params![next_start],
    ) {
        warn!("Failed to close open sessions before insert: {}", err);
    }

    sanitize_session_records(conn);
}

fn normalize_session_timeline(conn: &Connection) {
    let rows = match conn.prepare(
        "SELECT id, start_time, end_time
         FROM sessions
         ORDER BY start_time ASC, COALESCE(end_time, start_time) ASC, id ASC",
    ) {
        Ok(mut stmt) => stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map(|rows| rows.filter_map(Result::ok).collect::<Vec<_>>())
            .unwrap_or_default(),
        Err(err) => {
            warn!("Failed to scan sessions for overlap repair: {}", err);
            return;
        }
    };

    if rows.len() < 2 {
        return;
    }

    let mut repairs = Vec::new();

    for window in rows.windows(2) {
        let (id, start_text, end_text) = &window[0];
        let (_, next_start_text, _) = &window[1];

        let Some(start_time) = parse_rfc3339(start_text) else {
            continue;
        };
        let Some(end_time) = end_text.as_deref().and_then(parse_rfc3339) else {
            continue;
        };
        let Some(next_start) = parse_rfc3339(next_start_text) else {
            continue;
        };

        if end_time <= next_start {
            continue;
        }

        let repaired_end = next_start.max(start_time);
        let duration_secs = (repaired_end - start_time).num_seconds().max(0);
        repairs.push((id.clone(), repaired_end.to_rfc3339(), duration_secs));
    }

    if repairs.is_empty() {
        return;
    }

    warn!("Repairing {} overlapping session rows", repairs.len());

    for (id, end_time, duration_secs) in repairs {
        if let Err(err) = conn.execute(
            "UPDATE sessions
             SET end_time = ?1, duration_secs = ?2
             WHERE id = ?3",
            params![end_time, duration_secs, id],
        ) {
            warn!("Failed to repair overlap for session {}: {}", id, err);
        }
    }
}

fn parse_rfc3339(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_session_timeline_trims_overlaps() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL DEFAULT '',
                start_time TEXT NOT NULL,
                end_time TEXT,
                duration_secs INTEGER DEFAULT 0,
                is_idle INTEGER DEFAULT 0,
                date TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO sessions (id, app_name, window_title, start_time, end_time, duration_secs, date)
             VALUES ('a', 'kitty', '', '2026-04-23T10:00:00+05:30', '2026-04-23T10:30:00+05:30', 1800, '2026-04-23')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, app_name, window_title, start_time, end_time, duration_secs, date)
             VALUES ('b', 'code', '', '2026-04-23T10:15:00+05:30', '2026-04-23T10:45:00+05:30', 1800, '2026-04-23')",
            [],
        )
        .unwrap();

        normalize_session_timeline(&conn);

        let repaired: (String, i64) = conn
            .query_row(
                "SELECT end_time, duration_secs FROM sessions WHERE id = 'a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(repaired.0, "2026-04-23T10:15:00+05:30");
        assert_eq!(repaired.1, 900);
    }
}

fn end_of_day(date_text: &str, offset: &FixedOffset) -> Option<DateTime<FixedOffset>> {
    let day = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").ok()?;
    let naive = day.and_hms_opt(23, 59, 59)?;
    offset
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Some(offset.from_utc_datetime(&naive)))
}

// Allow Storage to be shared across threads via Arc
unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}
