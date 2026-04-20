use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{BarChart, Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

mod db;
mod consent;
use db::TuiData;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(err) = res { eprintln!("Error: {:?}", err); }
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    if !consent::has_consent() { return run_consent(terminal); }
    if !consent::is_daemon_running() { let _ = consent::start_daemon(); }
    run_dashboard(terminal)
}

fn run_consent(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut selected = false;
    loop {
        terminal.draw(|f| {
            let area = f.area();
            if area.width < 50 || area.height < 12 { return; }
            let chunks = Layout::default().direction(Direction::Vertical)
                .constraints([Constraint::Length(4), Constraint::Length(10), Constraint::Min(1)])
                .split(area);
            
            let title = Paragraph::new(" ═══ Mono ═══ ").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).border_style(Color::Cyan));
            f.render_widget(title, chunks[0]);
            
            let text = vec!["".into(), "  Screen Time Tracker".into(), "".into(),
                "  Mono helps you understand and improve your digital habits.".into(),
                "".into(), "  • Tracks active applications and windows".into(),
                "  • Stores all data locally (privacy-first)".into(),
                "  • Runs silently on system startup".into()];
            let desc = Paragraph::new(text).style(Style::default().fg(Color::White)).block(Block::default());
            f.render_widget(desc, chunks[1]);
            
            let opt1 = if !selected { "▶ Enable Tracking" } else { "  Enable Tracking" };
            let opt2 = if selected { "▶ Skip for Now" } else { "  Skip for Now" };
            let opts = vec![opt1.into(), opt2.into(), "".into(), " [Tab] toggle  [Enter] confirm  [q] quit".into()];
            let color = if !selected { Color::Green } else { Color::Yellow };
            let opts_p = Paragraph::new(opts).style(Style::default().fg(Color::White))
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
            if area.width < 40 || area.height < 10 { return; }
            let chunks = Layout::default().direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(10), Constraint::Min(6), Constraint::Length(5), Constraint::Length(1)])
                .split(area);
            render_header(f, chunks[0], &state.data);
            render_weekly(f, chunks[1], &state.data);
            render_apps(f, chunks[2], &state.data, state.selected_app);
            render_live(f, chunks[3], &state.data, state.tick_count);
            render_footer(f, chunks[4]);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('r') => state.refresh_data(),
                        KeyCode::Char('j') | KeyCode::Down => state.scroll_down(),
                        KeyCode::Char('k') | KeyCode::Up => state.scroll_up(),
                        KeyCode::Char('h') | KeyCode::Left => state.prev_day(),
                        KeyCode::Char('l') | KeyCode::Right => state.next_day(),
                        KeyCode::Char('g') => state.selected_day = 6,
                        _ => {}
                    }
                }
            }
        }
        state.tick_count = state.tick_count.wrapping_add(1);
        if state.tick_count % 20 == 0 { state.refresh_data(); }
    }
}

struct DashboardState {
    data: TuiData,
    selected_app: usize,
    selected_day: usize,
    tick_count: usize,
}

impl DashboardState {
    fn new() -> Self {
        let mut data = TuiData::new();
        data.refresh();
        Self { data, selected_app: 0, selected_day: 6, tick_count: 0 }
    }
    fn refresh_data(&mut self) { self.data.refresh(); self.data.refresh_live(); }
    fn scroll_down(&mut self) {
        let max = self.data.apps.len().saturating_sub(1);
        if self.selected_app < max { self.selected_app += 1; }
    }
    fn scroll_up(&mut self) { if self.selected_app > 0 { self.selected_app -= 1; } }
    fn prev_day(&mut self) { if self.selected_day > 0 { self.selected_day -= 1; } }
    fn next_day(&mut self) { if self.selected_day < 6 { self.selected_day += 1; } }
}

fn render_header(f: &mut Frame, area: Rect, data: &TuiData) {
    if area.width < 5 { return; }
    let total = format!("{}  │  {}", data.today_date, data.today_total);
    let p = Paragraph::new(total).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().title(" Mono ").borders(Borders::ALL).border_style(Color::Gray));
    f.render_widget(p, area);
}

fn render_weekly(f: &mut Frame, area: Rect, data: &TuiData) {
    let block = Paragraph::new("").block(Block::default()
        .title(" Weekly ")
        .borders(Borders::ALL)
        .border_style(Color::Gray));
    f.render_widget(block, area);
    
    let inner = Rect::new(area.x + 1, area.y + 1, area.x + area.width - 2, area.y + area.height - 1);
    if inner.width < 10 || inner.height < 3 { return; }
    
    if data.weekly.is_empty() {
        let empty = Paragraph::new("No data").style(Style::default().fg(Color::Gray)).block(Block::default());
        f.render_widget(empty, inner);
        return;
    }
    
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let bar_data: Vec<(&str, u64)> = data.weekly.iter().enumerate().map(|(i, d)| {
        let hours = (d.seconds as f64 / 3600.0) as u64;
        let label = *days.get(i).unwrap_or(&"");
        (label, hours)
    }).collect();
    
    let chart = BarChart::default()
        .data(&bar_data)
        .bar_width(4)
        .bar_gap(1);
    
    f.render_widget(chart, inner);
}

fn render_apps(f: &mut Frame, area: Rect, data: &TuiData, selected: usize) {
    let title = " Applications ";
    let p = Paragraph::new("").style(Style::default().fg(Color::White))
        .block(Block::default().title(title).borders(Borders::ALL).border_style(Color::Gray));
    f.render_widget(p, area);
    let inner = Rect::new(area.x + 1, area.y + 1, area.x + area.width - 2, area.y + area.height - 1);
    if inner.width < 10 || inner.height < 1 { return; }
    
    if data.apps.is_empty() {
        let msg = "No apps tracked yet";
        let x = inner.x + (inner.width.saturating_sub(msg.len() as u16)) / 2;
        f.render_widget(Paragraph::new(msg).style(Style::default().fg(Color::Gray)), Rect::new(x, inner.y + inner.height / 2, inner.x + inner.width - 1, inner.y + inner.height / 2 + 1));
        return;
    }
    
    let max = data.apps.iter().map(|a| a.seconds).max().unwrap_or(1);
    let bar_w = 10;
    
    for (i, app) in data.apps.iter().take(inner.height as usize).enumerate() {
        let y = inner.y + i as u16;
        let is_sel = i == selected;
        let pct = if max > 0 { (app.seconds as f64 / max as f64 * bar_w as f64) as usize } else { 0 };
        let filled = "█".repeat(pct.min(bar_w));
        let empty = "░".repeat(bar_w.saturating_sub(pct));
        
        let name = format!("{:<14}", app.name.chars().take(14).collect::<String>());
        let time = format!("{}m", app.seconds/60);
        let line = format!("{}  {}│{}  {}", name, filled, empty, time);
        
        let line_width = line.len() as u16;
        let x = inner.x + (inner.width.saturating_sub(line_width)) / 2;
        
        let name_style = if is_sel { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) };
        f.render_widget(Paragraph::new(line).style(name_style).block(Block::default()),
            Rect::new(x, y, x + line_width, y + 1));
    }
}

fn render_live(f: &mut Frame, area: Rect, data: &TuiData, tick: usize) {
    let p = Paragraph::new("").block(Block::default().title(" Live ").borders(Borders::ALL).border_style(Color::Gray));
    f.render_widget(p, area);
    let inner = Rect::new(area.x + 1, area.y + 1, area.x + area.width - 2, area.y + area.height - 1);
    if inner.width < 10 { return; }
    let blink = tick % 20 < 10;
    let ind = if blink && data.live_app.is_some() { "●" } else { "○" };
    let app = data.live_app.clone().unwrap_or_else(|| "Idle".to_string());
    let secs = data.live_seconds;
    let timer = format!("{:02}:{:02}:{:02}", secs/3600, (secs%3600)/60, secs%60);
    let live_color = if data.live_app.is_some() { Color::Green } else { Color::Gray };
    let live = format!(" {} {} │ {}", ind, app, timer);
    f.render_widget(Paragraph::new(live).style(Style::default().fg(live_color)).block(Block::default()),
        Rect::new(inner.x, inner.y, inner.x + inner.width.saturating_sub(1), inner.y + 1));
}

fn render_footer(f: &mut Frame, area: Rect) {
    if area.width < 10 { return; }
    let help_text = " j/k: scroll  h/l: days  g: today  r: refresh  q: quit ";
    let x = area.x + (area.width.saturating_sub(help_text.len() as u16)) / 2;
    let p = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray));
    f.render_widget(p, Rect::new(x, area.y, area.x + area.width - 1, area.y + 1));
}