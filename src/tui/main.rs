use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Sparkline, SparklineBar},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

mod consent;
mod db;
use db::TuiData;

const SECTION_WEEKLY: usize = 0;
const SECTION_APPS: usize = 1;

const PRIMARY: Color = Color::Rgb(136, 192, 208);
const ACCENT: Color = Color::Rgb(235, 203, 139);
const MUTED: Color = Color::Rgb(120, 128, 140);
const TEXT: Color = Color::Rgb(236, 239, 244);
const SURFACE: Color = Color::Rgb(59, 66, 82);
const ACTIVE: Color = Color::Rgb(163, 190, 140);
const WEEKLY_AXIS_MAX_HOURS: i64 = 10;
const WEEKLY_AXIS_MAX_SECONDS: i64 = WEEKLY_AXIS_MAX_HOURS * 3600;
const STATS_PANEL_HEIGHT: u16 = 5;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal);
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    if !consent::has_consent() {
        return run_consent(terminal);
    }
    if !consent::is_daemon_running() {
        let _ = consent::start_daemon();
    }
    run_dashboard(terminal)
}

fn run_consent(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut selected = false;
    loop {
        terminal.draw(|f| {
            let area = f.area();
            if area.width < 50 || area.height < 12 {
                return;
            }
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Length(10),
                    Constraint::Min(1),
                ])
                .split(area);

            let title = Paragraph::new(" ═══ Mono ═══ ")
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Color::Cyan),
                );
            f.render_widget(title, chunks[0]);

            let text = vec![
                "".into(),
                "  Screen Time Tracker".into(),
                "".into(),
                "  Mono helps you understand and improve your digital habits.".into(),
                "".into(),
                "  • Tracks active applications and windows".into(),
                "  • Stores all data locally (privacy-first)".into(),
                "  • Runs silently on system startup".into(),
            ];
            let desc = Paragraph::new(text)
                .style(Style::default().fg(Color::White))
                .block(Block::default());
            f.render_widget(desc, chunks[1]);

            let opt1 = if !selected {
                "▶ Enable Tracking"
            } else {
                "  Enable Tracking"
            };
            let opt2 = if selected {
                "▶ Skip for Now"
            } else {
                "  Skip for Now"
            };
            let opts = vec![
                opt1.into(),
                opt2.into(),
                "".into(),
                " [Tab] toggle  [Enter] confirm  [q] quit".into(),
            ];
            let color = if !selected {
                Color::Green
            } else {
                Color::Yellow
            };
            let opts_p = Paragraph::new(opts)
                .style(Style::default().fg(Color::White))
                .block(Block::default().borders(Borders::ALL).border_style(color));
            f.render_widget(opts_p, chunks[2]);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Tab => selected = !selected,
                        KeyCode::Enter => {
                            if !selected {
                                let _ = consent::set_consent(true);
                                let _ = consent::setup_autostart();
                                let _ = consent::start_daemon();
                            }
                            return run_dashboard(terminal);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn run_dashboard(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut state = DashboardState::new();
    loop {
        terminal.draw(|f| {
            let area = f.area();
            if area.width < 72 || area.height < 18 {
                render_too_small(f, area);
                return;
            }

            let outer = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(12),
                    Constraint::Length(1),
                ])
                .split(area);

            render_header(f, outer[0], &state.data, state.selected_day);
            let show_trend = should_render_trend(&state.data);

            if outer[1].width >= 100 {
                let columns = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
                    .split(outer[1]);
                let left = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(10), Constraint::Length(5)])
                    .split(columns[0]);
                let (apps_area, stats_area, trend_area) =
                    split_right_panel(columns[1], state.data.day_stats.is_some(), show_trend);

                render_weekly(
                    f,
                    left[0],
                    &state.data,
                    state.selected_day,
                    state.focused_section == SECTION_WEEKLY,
                );
                render_live(f, left[1], &state.data, state.tick_count);
                render_apps(
                    f,
                    apps_area,
                    &state.data,
                    state.selected_day,
                    state.selected_app,
                    state.focused_section == SECTION_APPS,
                );
                if let Some(stats_area) = stats_area {
                    render_stats(f, stats_area, &state.data);
                }
                if let Some(trend_area) = trend_area {
                    render_trend(
                        f,
                        trend_area,
                        &state.data,
                        state.selected_day,
                        state.selected_app,
                    );
                }
            } else {
                let stack = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(10),
                        Constraint::Length(5),
                        Constraint::Min(7),
                    ])
                    .split(outer[1]);
                let (apps_area, stats_area, trend_area) =
                    split_right_panel(stack[2], state.data.day_stats.is_some(), show_trend);

                render_weekly(
                    f,
                    stack[0],
                    &state.data,
                    state.selected_day,
                    state.focused_section == SECTION_WEEKLY,
                );
                render_live(f, stack[1], &state.data, state.tick_count);
                render_apps(
                    f,
                    apps_area,
                    &state.data,
                    state.selected_day,
                    state.selected_app,
                    state.focused_section == SECTION_APPS,
                );
                if let Some(stats_area) = stats_area {
                    render_stats(f, stats_area, &state.data);
                }
                if let Some(trend_area) = trend_area {
                    render_trend(
                        f,
                        trend_area,
                        &state.data,
                        state.selected_day,
                        state.selected_app,
                    );
                }
            }

            render_footer(f, outer[2]);
        })?;
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('r') => state.refresh_data(),
                        KeyCode::Char('j') | KeyCode::Down => {
                            state.scroll_down();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            state.scroll_up();
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            state.prev_day();
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            state.next_day();
                        }
                        KeyCode::Char('g') => {
                            state.selected_day = state.data.weekly.len().saturating_sub(1);
                            state.selected_app = 0;
                            state.sync_context();
                        }
                        KeyCode::Tab => state.next_section(),
                        KeyCode::Backspace => state.prev_section(),
                        _ => {}
                    }
                }
            }
        }
        state.sync_context();
        std::thread::sleep(Duration::from_millis(100));
        state.tick_count = state.tick_count.wrapping_add(1);
        if state.tick_count % 20 == 0 {
            state.refresh_data();
        }
    }
}

struct DashboardState {
    data: TuiData,
    selected_app: usize,
    selected_day: usize,
    tick_count: usize,
    focused_section: usize,
}

impl DashboardState {
    fn new() -> Self {
        let mut data = TuiData::new();
        data.refresh();
        data.refresh_live();
        let selected_day = data.weekly.len().saturating_sub(1);
        let mut state = Self {
            data,
            selected_app: 0,
            selected_day,
            tick_count: 0,
            focused_section: SECTION_APPS,
        };
        state.sync_context();
        state
    }

    fn refresh_data(&mut self) {
        self.data.refresh();
        self.data.refresh_live();
        self.sync_context();
    }

    fn sync_context(&mut self) {
        let max_day = self.data.weekly.len().saturating_sub(1);
        self.selected_day = self.selected_day.min(max_day);
        self.data.load_day(self.selected_day);

        let max_app = self.data.apps.len().saturating_sub(1);
        self.selected_app = self.selected_app.min(max_app);
        let selected_app = self
            .data
            .apps
            .get(self.selected_app)
            .map(|app| app.name.clone());
        self.data.refresh_app_trend(selected_app.as_deref());
    }

    fn scroll_down(&mut self) {
        if self.focused_section != SECTION_APPS {
            return;
        }
        let max = self.data.apps.len().saturating_sub(1);
        if self.selected_app < max {
            self.selected_app += 1;
            self.sync_context();
        }
    }

    fn scroll_up(&mut self) {
        if self.focused_section != SECTION_APPS {
            return;
        }
        if self.selected_app > 0 {
            self.selected_app -= 1;
            self.sync_context();
        }
    }

    fn prev_day(&mut self) {
        if self.focused_section != SECTION_WEEKLY {
            return;
        }
        if self.selected_day > 0 {
            self.selected_day -= 1;
            self.selected_app = 0;
            self.sync_context();
        }
    }

    fn next_day(&mut self) {
        if self.focused_section != SECTION_WEEKLY {
            return;
        }
        if self.selected_day < self.data.weekly.len().saturating_sub(1) {
            self.selected_day += 1;
            self.selected_app = 0;
            self.sync_context();
        }
    }

    fn next_section(&mut self) {
        self.focused_section = if self.focused_section == SECTION_WEEKLY {
            SECTION_APPS
        } else {
            SECTION_WEEKLY
        };
    }

    fn prev_section(&mut self) {
        self.next_section();
    }
}

fn render_too_small(f: &mut Frame, area: Rect) {
    let message = Paragraph::new("Mono needs at least 72x18 to render the dashboard cleanly.")
        .style(Style::default().fg(MUTED))
        .block(
            Block::default()
                .title(" Mono ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(MUTED)),
        );
    f.render_widget(message, area);
}

fn render_header(f: &mut Frame, area: Rect, data: &TuiData, selected_day: usize) {
    let selected = data.weekly.get(selected_day);
    let selected_label = selected
        .map(|day| format!("{} {}", day.label, format_duration(day.seconds)))
        .unwrap_or_else(|| "No selection".to_string());

    let line = Line::from(vec![
        Span::styled(
            data.today_date.as_str(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  |  ", Style::default().fg(MUTED)),
        Span::styled(
            format!("Today {}", data.today_total),
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  |  ", Style::default().fg(MUTED)),
        Span::styled(
            format!("Avg {}", format_duration(data.weekly_average_seconds())),
            Style::default().fg(MUTED),
        ),
        Span::styled("  |  ", Style::default().fg(MUTED)),
        Span::styled(
            format!("Selected {}", selected_label),
            Style::default().fg(ACCENT),
        ),
    ]);

    let header = Paragraph::new(line).block(
        Block::default()
            .title(Line::from(Span::styled(
                " Mono ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED)),
    );
    f.render_widget(header, area);
}

fn render_weekly(f: &mut Frame, area: Rect, data: &TuiData, selected_day: usize, focused: bool) {
    let border_color = if focused { ACCENT } else { MUTED };
    let selected = data.weekly.get(selected_day);
    let subtitle = selected
        .map(|day| format!(" {} {}", day.label, format_duration(day.seconds)))
        .unwrap_or_else(|| " No data".to_string());
    let title = Line::from(vec![
        Span::styled(
            " Weekly ",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(subtitle, Style::default().fg(ACCENT)),
    ]);

    let inner = render_panel(f, area, title, border_color);
    if inner.width < 18 || inner.height < 6 {
        return;
    }

    if data.weekly.is_empty() {
        let empty = Paragraph::new("No weekly data yet.").style(Style::default().fg(MUTED));
        f.render_widget(empty, inner);
        return;
    }

    if data.weekly.len() == 1 {
        if let Some(day) = data.weekly.first() {
            render_single_day_summary(f.buffer_mut(), inner, day);
        }
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(1)])
        .split(inner);
    let chart_area = sections[0];
    let labels_area = sections[1];

    let chart_parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(chart_area);

    let slots = calculate_bar_slots(chart_parts[1], data.weekly.len());
    draw_weekly_plot(
        f.buffer_mut(),
        chart_parts[0],
        chart_parts[1],
        &data.weekly,
        &slots,
        selected_day,
    );
    draw_day_labels(
        f.buffer_mut(),
        labels_area,
        &data.weekly,
        &slots,
        selected_day,
    );
}

fn render_apps(
    f: &mut Frame,
    area: Rect,
    data: &TuiData,
    selected_day: usize,
    selected_app: usize,
    focused: bool,
) {
    let border_color = if focused { ACCENT } else { MUTED };
    let day_label = data
        .weekly
        .get(selected_day)
        .map(|day| format!(" {} {}", day.label, format_duration(day.seconds)))
        .unwrap_or_else(|| " Selected day".to_string());
    let title = Line::from(vec![
        Span::styled(
            " Applications ",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(day_label, Style::default().fg(PRIMARY)),
    ]);
    let inner = render_panel(f, area, title, border_color);
    if inner.width < 18 || inner.height < 3 {
        return;
    }

    if data.apps.is_empty() {
        let empty = Paragraph::new("No active apps recorded for the selected day.")
            .style(Style::default().fg(MUTED));
        f.render_widget(empty, inner);
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    render_apps_header(f.buffer_mut(), sections[0]);

    let rows = sections[1].height as usize;
    let max_value = data.apps.iter().map(|app| app.seconds).max().unwrap_or(1);
    for (index, app) in data.apps.iter().take(rows).enumerate() {
        let row = Rect::new(
            sections[1].x,
            sections[1].y + index as u16,
            sections[1].width,
            1,
        );
        render_app_row(
            f.buffer_mut(),
            row,
            app.name.as_str(),
            app.seconds,
            max_value,
            index == 0,
            index == selected_app,
        );
    }
}

fn render_stats(f: &mut Frame, area: Rect, data: &TuiData) {
    let Some(stats) = data.day_stats.as_ref() else {
        return;
    };

    let inner = render_panel(
        f,
        area,
        Line::from(Span::styled(
            " Stats ",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        )),
        MUTED,
    );
    if inner.width < 18 || inner.height < 3 {
        return;
    }

    let lines = vec![
        stats_line("Apps used:", stats.apps_used.to_string()),
        stats_line(
            "Peak hour:",
            stats
                .peak_hour
                .clone()
                .unwrap_or_else(|| "None".to_string()),
        ),
        stats_line("Focus time:", format_duration(stats.focus_time_seconds)),
    ];
    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_live(f: &mut Frame, area: Rect, data: &TuiData, tick: usize) {
    let inner = render_panel(
        f,
        area,
        Line::from(Span::styled(
            " Now ",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        MUTED,
    );
    if inner.width < 10 || inner.height == 0 {
        return;
    }

    let blink = tick % 20 < 10;
    let ind = if blink && data.live_app.is_some() {
        "●"
    } else {
        "○"
    };
    let app = data.live_app.clone().unwrap_or_else(|| "Idle".to_string());
    let secs = data.live_seconds;
    let timer = format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    );
    let live_color = if data.live_app.is_some() {
        ACTIVE
    } else {
        MUTED
    };

    let lines = if inner.height >= 2 {
        vec![
            Line::from(vec![
                Span::styled(ind, Style::default().fg(live_color)),
                Span::raw(" "),
                Span::styled(
                    truncate_text(&app, inner.width.saturating_sub(2) as usize),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("elapsed ", Style::default().fg(MUTED)),
                Span::styled(timer, Style::default().fg(live_color)),
            ]),
        ]
    } else {
        vec![Line::from(vec![
            Span::styled(ind, Style::default().fg(live_color)),
            Span::raw(" "),
            Span::styled(
                format!("{} {}", truncate_text(&app, 18), timer),
                Style::default().fg(TEXT),
            ),
        ])]
    };

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_trend(
    f: &mut Frame,
    area: Rect,
    data: &TuiData,
    selected_day: usize,
    selected_app: usize,
) {
    let trend_label = data
        .apps
        .get(selected_app)
        .map(|app| truncate_text(&app.name, 16))
        .unwrap_or_else(|| "selected app".to_string());
    let title = Line::from(vec![
        Span::styled(
            " Trend ",
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", trend_label), Style::default().fg(ACCENT)),
    ]);
    let inner = render_panel(f, area, title, MUTED);
    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(inner);

    let slots = calculate_compact_slots(sections[0], data.weekly.len());
    let max_value = data.app_trend.iter().copied().max().unwrap_or(1).max(1);
    let sparkline_data = expand_sparkline_data(data, &slots, selected_day);
    let sparkline = Sparkline::default().data(sparkline_data).max(max_value);
    f.render_widget(sparkline, sections[0]);
    draw_day_labels(
        f.buffer_mut(),
        sections[1],
        &data.weekly,
        &slots,
        selected_day,
    );
}

fn render_footer(f: &mut Frame, area: Rect) {
    let help_text = "Tab section  j/k apps  h/l days  g today  r refresh  q quit";
    let footer = Paragraph::new(help_text).style(Style::default().fg(MUTED));
    f.render_widget(footer, area);
}

fn render_panel(f: &mut Frame, area: Rect, title: Line<'_>, border_color: Color) -> Rect {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

fn render_single_day_summary(buf: &mut Buffer, area: Rect, day: &db::DayData) {
    if area.width < 12 || area.height == 0 {
        return;
    }

    let label = if day.is_today {
        "Today"
    } else {
        day.label.as_str()
    };
    let value = format_duration(day.seconds);
    let prefix = format!("{label}: ");
    let y = area.y + area.height.saturating_sub(1) / 2;

    let bar_space = area
        .width
        .saturating_sub(prefix.len() as u16)
        .saturating_sub(value.len() as u16)
        .saturating_sub(1) as usize;
    let fill_ratio = (day.seconds as f64 / WEEKLY_AXIS_MAX_SECONDS as f64).clamp(0.0, 1.0);
    let filled = if day.seconds > 0 && bar_space > 0 {
        ((fill_ratio * bar_space as f64).round() as usize).clamp(1, bar_space)
    } else {
        0
    };
    let bar = "█".repeat(filled);

    buf.set_string(area.x, y, &prefix, Style::default().fg(MUTED));
    buf.set_string(
        area.x + prefix.len() as u16,
        y,
        bar,
        Style::default().fg(ACCENT),
    );
    let value_x = area.right().saturating_sub(value.len() as u16);
    buf.set_string(value_x, y, value, Style::default().fg(TEXT));
}

fn render_apps_header(buf: &mut Buffer, area: Rect) {
    if area.width < 12 || area.height == 0 {
        return;
    }

    let parts = app_row_parts(area);
    buf.set_string(parts[0].x, area.y, "App", Style::default().fg(MUTED));
    buf.set_string(parts[1].x, area.y, "Usage", Style::default().fg(MUTED));

    let label = "Time";
    let x = parts[2].right().saturating_sub(label.len() as u16);
    buf.set_string(x, area.y, label, Style::default().fg(MUTED));
}

fn render_app_row(
    buf: &mut Buffer,
    area: Rect,
    name: &str,
    seconds: i64,
    max_value: i64,
    is_top_app: bool,
    selected: bool,
) {
    if area.width < 12 || area.height == 0 {
        return;
    }

    if selected {
        buf.set_style(area, Style::default().bg(SURFACE));
    }

    let parts = app_row_parts(area);
    let name_style = if selected {
        Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
    } else if is_top_app {
        Style::default().fg(TEXT)
    } else {
        Style::default().fg(MUTED)
    };
    let bar_color = if selected || is_top_app {
        ACCENT
    } else {
        MUTED
    };
    let bar_style = Style::default().fg(bar_color);
    let bar_bg_style = Style::default().fg(SURFACE);
    let value_style = if selected || is_top_app {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(MUTED)
    };

    let name = truncate_text(name, parts[0].width as usize);
    buf.set_stringn(
        parts[0].x,
        area.y,
        name,
        parts[0].width as usize,
        name_style,
    );

    let bar_width = parts[1].width;
    if bar_width > 0 {
        let ratio = if max_value > 0 {
            seconds as f64 / max_value as f64
        } else {
            0.0
        };
        let filled = (ratio * bar_width as f64).round() as u16;
        for offset in 0..bar_width {
            let style = if offset < filled {
                bar_style
            } else {
                bar_bg_style
            };
            let symbol = if offset < filled { "█" } else { "░" };
            buf[(parts[1].x + offset, area.y)]
                .set_symbol(symbol)
                .set_style(style);
        }
    }

    let value = format_duration(seconds);
    let x = parts[2].right().saturating_sub(value.len() as u16);
    buf.set_stringn(x, area.y, value, parts[2].width as usize, value_style);
}

fn stats_line(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<11}"), Style::default().fg(MUTED)),
        Span::styled(value, Style::default().fg(TEXT)),
    ])
}

fn draw_weekly_plot(
    buf: &mut Buffer,
    y_axis_area: Rect,
    plot_area: Rect,
    days: &[db::DayData],
    slots: &[(u16, u16)],
    selected_day: usize,
) {
    if plot_area.width == 0 || plot_area.height == 0 {
        return;
    }

    for tick_hour in (0..=WEEKLY_AXIS_MAX_HOURS).step_by(2) {
        let tick_seconds = tick_hour * 3600;
        let y = value_row(tick_seconds, WEEKLY_AXIS_MAX_SECONDS, plot_area);
        let label = format!("{tick_hour}h");
        let label_x = y_axis_area.right().saturating_sub(label.len() as u16);
        buf.set_string(label_x, y, label, Style::default().fg(MUTED));

        for x in plot_area.left()..plot_area.right() {
            let symbol = if tick_hour == 0 { "─" } else { "┈" };
            buf[(x, y)]
                .set_symbol(symbol)
                .set_style(Style::default().fg(SURFACE));
        }
    }

    for (index, day) in days.iter().enumerate() {
        let Some((start_x, width)) = slots.get(index).copied() else {
            continue;
        };
        let filled_rows = value_height(day.seconds, WEEKLY_AXIS_MAX_SECONDS, plot_area.height);
        if filled_rows == 0 {
            continue;
        }

        let style = Style::default().fg(if index == selected_day {
            ACCENT
        } else if day.is_today {
            PRIMARY
        } else {
            MUTED
        });
        let top = plot_area.bottom().saturating_sub(filled_rows);
        for y in top..plot_area.bottom() {
            for x in start_x..start_x.saturating_add(width) {
                if x < plot_area.right() {
                    buf[(x, y)].set_symbol("█").set_style(style);
                }
            }
        }
    }
}

fn split_right_panel(
    area: Rect,
    has_stats: bool,
    show_trend: bool,
) -> (Rect, Option<Rect>, Option<Rect>) {
    match (has_stats, show_trend) {
        (true, true) => {
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Length(STATS_PANEL_HEIGHT),
                    Constraint::Min(0),
                ])
                .split(area);
            (sections[0], Some(sections[1]), Some(sections[2]))
        }
        (true, false) => {
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(STATS_PANEL_HEIGHT)])
                .split(area);
            (sections[0], Some(sections[1]), None)
        }
        (false, true) => {
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(0)])
                .split(area);
            (sections[0], None, Some(sections[1]))
        }
        (false, false) => (area, None, None),
    }
}

fn should_render_trend(data: &TuiData) -> bool {
    data.app_trend.len() >= 2 && data.app_trend.iter().filter(|&&value| value > 0).count() > 1
}

fn draw_day_labels(
    buf: &mut Buffer,
    area: Rect,
    days: &[db::DayData],
    slots: &[(u16, u16)],
    selected_day: usize,
) {
    if area.height == 0 {
        return;
    }

    for (index, day) in days.iter().enumerate() {
        let Some((start_x, width)) = slots.get(index).copied() else {
            continue;
        };
        let label = if width >= 3 {
            day.label.clone()
        } else {
            day.label.chars().take(1).collect::<String>()
        };
        let label_len = label.len() as u16;
        let centered_x = start_x + width.saturating_sub(label_len) / 2;
        let label_x = centered_x.clamp(area.x, area.right().saturating_sub(label_len));
        let style = if index == selected_day {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else if day.is_today {
            Style::default().fg(PRIMARY)
        } else {
            Style::default().fg(MUTED)
        };
        buf.set_stringn(
            label_x,
            area.y,
            label,
            area.right().saturating_sub(label_x) as usize,
            style,
        );
    }
}

fn app_row_parts(area: Rect) -> [Rect; 3] {
    let value_width = if area.width >= 24 {
        8
    } else {
        area.width.saturating_sub(7).min(8).max(5)
    };
    let name_width = if area.width >= 24 {
        12
    } else {
        area.width
            .saturating_sub(value_width)
            .saturating_sub(1)
            .min(12)
            .max(6)
    };
    let constraints = [
        Constraint::Length(name_width),
        Constraint::Min(0),
        Constraint::Length(value_width),
    ];
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);
    [parts[0], parts[1], parts[2]]
}

fn calculate_bar_slots(area: Rect, count: usize) -> Vec<(u16, u16)> {
    if area.width == 0 || count == 0 {
        return Vec::new();
    }

    let count = count as u16;
    let gap: u16 = if area.width >= count.saturating_mul(5) {
        2
    } else {
        1
    };
    let total_gap = gap.saturating_mul(count.saturating_sub(1));
    let available = area.width.saturating_sub(total_gap);
    let bar_width = (available / count).max(1);
    let used = bar_width.saturating_mul(count) + total_gap;
    let mut x = area.x + area.width.saturating_sub(used) / 2;
    let mut slots = Vec::with_capacity(count as usize);

    for _ in 0..count {
        slots.push((x, bar_width));
        x = x.saturating_add(bar_width).saturating_add(gap);
    }

    slots
}

fn calculate_compact_slots(area: Rect, count: usize) -> Vec<(u16, u16)> {
    if area.width == 0 || count == 0 {
        return Vec::new();
    }

    let base = area.width / count as u16;
    let remainder = area.width % count as u16;
    let mut x = area.x;
    let mut slots = Vec::with_capacity(count);

    for index in 0..count as u16 {
        let width = base + u16::from(index < remainder);
        slots.push((x, width.max(1)));
        x = x.saturating_add(width.max(1));
    }

    slots
}

fn expand_sparkline_data(
    data: &TuiData,
    slots: &[(u16, u16)],
    selected_day: usize,
) -> Vec<SparklineBar> {
    let mut sparkline = Vec::new();
    for (index, (_, width)) in slots.iter().copied().enumerate() {
        let value = data.app_trend.get(index).copied().unwrap_or(0);
        let style = Some(
            Style::default().fg(if index == selected_day {
                ACCENT
            } else if data
                .weekly
                .get(index)
                .map(|day| day.is_today)
                .unwrap_or(false)
            {
                PRIMARY
            } else {
                MUTED
            }),
        );

        for _ in 0..width {
            sparkline.push(SparklineBar::from(value).style(style));
        }
    }
    sparkline
}

fn value_height(value: i64, max_value: i64, height: u16) -> u16 {
    if value <= 0 || max_value <= 0 || height == 0 {
        return 0;
    }

    let ratio = (value as f64 / max_value as f64).clamp(0.0, 1.0);
    let filled = (ratio * height as f64).ceil() as u16;
    filled.clamp(1, height)
}

fn value_row(value: i64, max_value: i64, area: Rect) -> u16 {
    let height = value_height(value, max_value, area.height);
    if height == 0 {
        area.bottom().saturating_sub(1)
    } else {
        area.bottom().saturating_sub(height)
    }
}

fn truncate_text(text: &str, max_width: usize) -> String {
    if text.chars().count() <= max_width {
        return text.to_string();
    }

    if max_width <= 2 {
        return text.chars().take(max_width).collect();
    }

    let mut truncated: String = text.chars().take(max_width - 2).collect();
    truncated.push_str("..");
    truncated
}

fn format_duration(secs: i64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn dashboard_renders_wide_terminal_without_panicking() {
        render_dashboard_at_size(173, 46);
    }

    #[test]
    fn dashboard_renders_across_multiple_terminal_sizes() {
        for (width, height) in [(72, 18), (88, 22), (100, 28), (121, 34), (173, 46)] {
            render_dashboard_at_size(width, height);
        }
    }

    fn render_dashboard_at_size(width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let data = sample_data();

        terminal
            .draw(|f| draw_dashboard_for_test(f, &data))
            .unwrap();
    }

    fn draw_dashboard_for_test(f: &mut Frame, data: &TuiData) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(12),
                Constraint::Length(1),
            ])
            .split(f.area());

        render_header(f, outer[0], data, 6);
        let show_trend = should_render_trend(data);

        if outer[1].width >= 100 {
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
                .split(outer[1]);
            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(5)])
                .split(columns[0]);
            let (apps_area, stats_area, trend_area) =
                split_right_panel(columns[1], data.day_stats.is_some(), show_trend);

            render_weekly(f, left[0], data, 6, true);
            render_live(f, left[1], data, 0);
            render_apps(f, apps_area, data, 6, 0, true);
            if let Some(stats_area) = stats_area {
                render_stats(f, stats_area, data);
            }
            if let Some(trend_area) = trend_area {
                render_trend(f, trend_area, data, 6, 0);
            }
        } else {
            let stack = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(5),
                    Constraint::Min(7),
                ])
                .split(outer[1]);
            let (apps_area, stats_area, trend_area) =
                split_right_panel(stack[2], data.day_stats.is_some(), show_trend);

            render_weekly(f, stack[0], data, 6, true);
            render_live(f, stack[1], data, 0);
            render_apps(f, apps_area, data, 6, 0, true);
            if let Some(stats_area) = stats_area {
                render_stats(f, stats_area, data);
            }
            if let Some(trend_area) = trend_area {
                render_trend(f, trend_area, data, 6, 0);
            }
        }

        render_footer(f, outer[2]);
    }

    fn sample_data() -> TuiData {
        let mut data = TuiData::new();
        data.today_date = "2026-04-22".to_string();
        data.today_total = "2h 8m".to_string();
        data.weekly = vec![
            db::DayData {
                label: "Mon".to_string(),
                date: "2026-04-16".to_string(),
                seconds: 20 * 60,
                is_today: false,
            },
            db::DayData {
                label: "Tue".to_string(),
                date: "2026-04-17".to_string(),
                seconds: 35 * 60,
                is_today: false,
            },
            db::DayData {
                label: "Wed".to_string(),
                date: "2026-04-18".to_string(),
                seconds: 22 * 60,
                is_today: false,
            },
            db::DayData {
                label: "Thu".to_string(),
                date: "2026-04-19".to_string(),
                seconds: 42 * 60,
                is_today: false,
            },
            db::DayData {
                label: "Fri".to_string(),
                date: "2026-04-20".to_string(),
                seconds: 10 * 3600,
                is_today: false,
            },
            db::DayData {
                label: "Sat".to_string(),
                date: "2026-04-21".to_string(),
                seconds: 38 * 60,
                is_today: false,
            },
            db::DayData {
                label: "Sun".to_string(),
                date: "2026-04-22".to_string(),
                seconds: 83 * 60,
                is_today: true,
            },
        ];
        data.apps = vec![
            db::AppData {
                name: "kitty".to_string(),
                seconds: 83 * 60,
            },
            db::AppData {
                name: "helium".to_string(),
                seconds: 44 * 60,
            },
            db::AppData {
                name: "opencode-with-a-surprisingly-long-name".to_string(),
                seconds: 21 * 60,
            },
        ];
        data.day_stats = Some(db::DayStats {
            apps_used: 6,
            peak_hour: Some("3 PM".to_string()),
            focus_time_seconds: 4 * 3600 + 12 * 60,
        });
        data.app_trend = vec![0, 15 * 60, 5 * 60, 25 * 60, 30 * 60, 40 * 60, 83 * 60]
            .into_iter()
            .map(|value| value as u64)
            .collect();
        data.live_app = Some("kitty".to_string());
        data.live_seconds = 83 * 60;
        data
    }
}
