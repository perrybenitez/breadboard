// P2.6: Interactive CLI Dashboard using ratatui

use crate::constants::SERVICES;
use crate::db::Database;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort cleanup is intentionally performed from Drop so terminal
        // state is restored even when drawing or input handling returns an
        // error.
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
    }
}

struct DashboardState {
    should_quit: bool,
}

impl DashboardState {
    fn new() -> Self {
        DashboardState { should_quit: false }
    }
}

pub fn run_dashboard(db: &Database) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _terminal_guard = TerminalGuard;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = DashboardState::new();

    // Main loop
    loop {
        terminal.draw(|f| {
            if let Err(e) = render_dashboard(f, db) {
                // If rendering fails, just show error
                let error_text = format!("Error: {}", e);
                let paragraph = Paragraph::new(error_text)
                    .block(Block::default().borders(Borders::ALL).title("Error"));
                f.render_widget(paragraph, f.size());
            }
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        state.should_quit = true;
                    }
                    KeyCode::Char('r') => {
                        // Refresh (redraw on next loop)
                    }
                    _ => {}
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    Ok(())
}

fn render_dashboard(f: &mut Frame, db: &Database) -> Result<()> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(10), // Service gauges
            Constraint::Min(5),     // History
            Constraint::Length(3),  // Help
        ])
        .split(f.size());

    // Title
    render_title(f, chunks[0]);

    // Service usage gauges
    render_service_usage(f, chunks[1], db)?;

    // Recent history
    render_history(f, chunks[2], db)?;

    // Help text
    render_help(f, chunks[3]);

    Ok(())
}

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new("Breadboard Dashboard")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn render_service_usage(f: &mut Frame, area: Rect, db: &Database) -> Result<()> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    for (idx, &service) in SERVICES.iter().enumerate() {
        let usage = db.get_usage(service).unwrap_or(0);
        let budget = db.get_budget(service).unwrap_or(1_000_000);

        let ratio = if budget > 0 {
            (usage as f64 / budget as f64).min(1.0)
        } else {
            0.0
        };

        let pct = (ratio * 100.0) as u16;

        // Color based on usage
        let color = if pct > 90 {
            Color::Red
        } else if pct > 70 {
            Color::Yellow
        } else {
            Color::Green
        };

        let label = format!(
            "{}: {}/{} ({pct}%)",
            service,
            format_tokens(usage),
            format_tokens(budget)
        );

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(service))
            .gauge_style(Style::default().fg(color))
            .ratio(ratio)
            .label(label);

        f.render_widget(gauge, chunks[idx]);
    }

    Ok(())
}

fn render_history(f: &mut Frame, area: Rect, db: &Database) -> Result<()> {
    let history = db.get_history(10).unwrap_or_default();

    let items: Vec<ListItem> = history
        .iter()
        .map(|entry| {
            let content = format!(
                "{} {} → {} ({})",
                entry.timestamp,
                entry.prompt_preview,
                entry.service,
                format_tokens(entry.tokens)
            );
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Recent History"),
    );

    f.render_widget(list, area);

    Ok(())
}

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![Line::from(vec![
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Quit  "),
        Span::styled(
            "r",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Refresh"),
    ])];

    let paragraph =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title("Controls"));

    f.render_widget(paragraph, area);
}

fn format_tokens(tokens: i64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}
