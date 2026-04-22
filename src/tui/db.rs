use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, TimeZone, Timelike};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
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

#[derive(Clone)]
struct SessionSpan {
    app_name: String,
    start: DateTime<FixedOffset>,
    end: DateTime<FixedOffset>,
}

#[derive(Clone, Copy)]
struct TimeRange {
    start: DateTime<FixedOffset>,
    end: DateTime<FixedOffset>,
}

impl TuiData {
    pub fn new() -> Self {
        let conn = Connection::open(Self::db_path())
            .unwrap_or_else(|_| Connection::open_in_memory().expect("Failed to open database"));
        sanitize_tui_session_records(&conn);

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
        let total = self.total_seconds_for_date(&today);

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

            let secs = self.total_seconds_for_date(&date_str);

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
        let now = Local::now().fixed_offset();
        let live = self
            .conn
            .query_row::<(String, String), _, _>(
                "SELECT app_name, start_time
                 FROM sessions
                 WHERE date = ?1
                   AND end_time IS NULL
                   AND is_idle = 0
                   AND TRIM(app_name) <> ''
                 ORDER BY start_time DESC
                 LIMIT 1",
                params![&today],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .unwrap_or(None);

        if let Some((app, seconds)) = live.and_then(|(app, start)| {
            parse_rfc3339(&start).map(|start_time| {
                let seconds = (now - start_time).num_seconds().max(0);
                (app, seconds)
            })
        }) {
            self.live_app = Some(app);
            self.live_seconds = seconds;
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
        let mut apps = self
            .seconds_by_app_for_date(date)
            .into_iter()
            .map(|(name, seconds)| AppData { name, seconds })
            .collect::<Vec<_>>();
        apps.sort_by(|left, right| {
            right
                .seconds
                .cmp(&left.seconds)
                .then_with(|| left.name.cmp(&right.name))
        });
        apps.truncate(12);
        apps
    }

    fn app_seconds_for_date(&self, date: &str, app_name: &str) -> i64 {
        self.seconds_by_app_for_date(date)
            .into_iter()
            .find(|(name, _)| name == app_name)
            .map(|(_, seconds)| seconds)
            .unwrap_or(0)
    }

    fn load_stats_for_date(&self, date: &str) -> Option<DayStats> {
        let by_app = self.seconds_by_app_for_date(date);
        let apps_used = by_app.values().filter(|&&seconds| seconds > 0).count();
        let merged_ranges = self.merged_ranges_for_date(date, None);
        let peak_hour = peak_hour_from_ranges(&merged_ranges);
        let focus_time_seconds = merged_ranges
            .iter()
            .map(TimeRange::duration_secs)
            .filter(|&seconds| seconds >= 25 * 60)
            .sum::<i64>();

        if apps_used == 0 && peak_hour.is_none() && focus_time_seconds <= 0 {
            None
        } else {
            Some(DayStats {
                apps_used,
                peak_hour,
                focus_time_seconds,
            })
        }
    }

    fn total_seconds_for_date(&self, date: &str) -> i64 {
        self.merged_ranges_for_date(date, None)
            .iter()
            .map(TimeRange::duration_secs)
            .sum()
    }

    fn seconds_by_app_for_date(&self, date: &str) -> HashMap<String, i64> {
        let mut grouped: HashMap<String, Vec<TimeRange>> = HashMap::new();
        for span in self.load_spans_for_date(date) {
            grouped.entry(span.app_name).or_default().push(TimeRange {
                start: span.start,
                end: span.end,
            });
        }

        grouped
            .into_iter()
            .filter_map(|(app_name, ranges)| {
                let total = merge_ranges(ranges)
                    .iter()
                    .map(TimeRange::duration_secs)
                    .sum::<i64>();
                if total > 0 {
                    Some((app_name, total))
                } else {
                    None
                }
            })
            .collect()
    }

    fn merged_ranges_for_date(&self, date: &str, app_name: Option<&str>) -> Vec<TimeRange> {
        let ranges = self
            .load_spans_for_date(date)
            .into_iter()
            .filter(|span| app_name.is_none_or(|name| span.app_name == name))
            .map(|span| TimeRange {
                start: span.start,
                end: span.end,
            })
            .collect::<Vec<_>>();
        merge_ranges(ranges)
    }

    fn load_spans_for_date(&self, date: &str) -> Vec<SessionSpan> {
        let now = Local::now().to_rfc3339();
        match self.conn.prepare(
            "SELECT app_name, start_time, COALESCE(end_time, ?2)
             FROM sessions
             WHERE date = ?1
               AND is_idle = 0
               AND TRIM(app_name) <> ''
             ORDER BY start_time ASC",
        ) {
            Ok(mut stmt) => stmt
                .query_map(params![date, now], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map(|rows| {
                    rows.filter_map(Result::ok)
                        .filter_map(|(app_name, start, end)| {
                            clamp_span_to_day(date, &app_name, &start, &end)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
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

impl TimeRange {
    fn duration_secs(&self) -> i64 {
        (self.end - self.start).num_seconds().max(0)
    }
}

fn clamp_span_to_day(
    date_text: &str,
    app_name: &str,
    start_text: &str,
    end_text: &str,
) -> Option<SessionSpan> {
    let start = parse_rfc3339(start_text)?;
    let end = parse_rfc3339(end_text)?;
    let day_end = end_of_day(date_text, start.offset())?;
    let clamped_end = end.min(day_end).max(start);
    if clamped_end <= start {
        return None;
    }

    Some(SessionSpan {
        app_name: app_name.to_string(),
        start,
        end: clamped_end,
    })
}

fn parse_rfc3339(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok()
}

fn sanitize_tui_session_records(conn: &Connection) {
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
        Err(_) => return,
    };

    if rows.is_empty() {
        return;
    }

    let now = Local::now().fixed_offset();
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

        let _ = conn.execute(
            "UPDATE sessions
             SET end_time = ?1, duration_secs = ?2
             WHERE id = ?3",
            params![repaired_end.to_rfc3339(), duration_secs, id],
        );
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

fn merge_ranges(mut ranges: Vec<TimeRange>) -> Vec<TimeRange> {
    if ranges.is_empty() {
        return Vec::new();
    }

    ranges.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));

    let mut merged: Vec<TimeRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end {
                last.end = last.end.max(range.end);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn peak_hour_from_ranges(ranges: &[TimeRange]) -> Option<String> {
    let mut buckets = [0_i64; 24];

    for range in ranges {
        let mut cursor = range.start;
        while cursor < range.end {
            let hour_end = next_hour(cursor).min(range.end);
            buckets[cursor.hour() as usize] += (hour_end - cursor).num_seconds().max(0);
            cursor = hour_end;
        }
    }

    buckets
        .iter()
        .enumerate()
        .max_by_key(|&(hour, seconds)| (seconds, -(hour as i64)))
        .and_then(|(hour, seconds)| (*seconds > 0).then(|| format_hour_label(hour as i64)))
}

fn next_hour(moment: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let next = moment + chrono::Duration::hours(1);
    moment
        .offset()
        .with_ymd_and_hms(next.year(), next.month(), next.day(), next.hour(), 0, 0)
        .single()
        .unwrap_or(next)
}
