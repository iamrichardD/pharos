/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/tui.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements the ratatui-based Terminal User Interface (TUI).
 * It provides a real-time visualization of server health, database size,
 * and live system events.
 * * Traceability:
 * Related to Task 12.2 (Issue #45)
 * ======================================================================== */

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self};
use std::time::Duration;
use tokio::time;
use tokio::sync::broadcast;
use lazy_static::lazy_static;

use crate::metrics::{CPU_USAGE, MEMORY_USAGE_BYTES, TOTAL_RECORDS};

lazy_static! {
    pub static ref EVENT_TX: broadcast::Sender<String> = {
        let (tx, _) = broadcast::channel(100);
        tx
    };
}

pub struct AppState {
    pub events: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            events: vec!["TUI initialized. Waiting for events...".to_string()],
        }
    }
}

pub async fn run_tui() -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new();
    let mut rx = EVENT_TX.subscribe();
    let mut interval = time::interval(Duration::from_millis(250));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                terminal.draw(|f| {
                    let size = f.area();
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3),
                            Constraint::Min(10),
                            Constraint::Length(3),
                        ])
                        .split(size);

                    // Header
                    let header = Paragraph::new(vec![Line::from(vec![
                        Span::styled("Pharos Server TUI ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw("- ONLINE"),
                    ])])
                    .block(Block::default().borders(Borders::ALL).title("Status"));
                    f.render_widget(header, chunks[0]);

                    let center_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(5),
                            Constraint::Min(5),
                        ])
                        .split(chunks[1]);

                    let metrics_stats_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(50),
                            Constraint::Percentage(50),
                        ])
                        .split(center_chunks[0]);

                    // Metrics Panel (Left)
                    let cpu = CPU_USAGE.get();
                    let mem = MEMORY_USAGE_BYTES.get() as f64 / (1024.0 * 1024.0);
                    let metrics_text = vec![
                        Line::from(format!("CPU Usage: {:.1}%", cpu)),
                        Line::from(format!("Memory: {:.1} MB", mem)),
                    ];
                    let metrics = Paragraph::new(metrics_text)
                        .block(Block::default().borders(Borders::ALL).title("Metrics"));
                    f.render_widget(metrics, metrics_stats_chunks[0]);

                    // Stats Panel (Right)
                    let records = TOTAL_RECORDS.get();
                    let stats_text = vec![
                        Line::from(format!("Total Records: {}", records)),
                    ];
                    let stats = Paragraph::new(stats_text)
                        .block(Block::default().borders(Borders::ALL).title("Database Stats"));
                    f.render_widget(stats, metrics_stats_chunks[1]);

                    // Event Stream Panel (Bottom)
                    let event_lines: Vec<Line> = state.events.iter()
                        .map(|e| Line::from(Span::raw(e)))
                        .collect();
                    let mut scroll_offset = 0;
                    if event_lines.len() as u16 > center_chunks[1].height.saturating_sub(2) {
                        scroll_offset = event_lines.len() as u16 - center_chunks[1].height.saturating_sub(2);
                    }
                    let events_widget = Paragraph::new(event_lines)
                        .block(Block::default().borders(Borders::ALL).title("Event Stream"))
                        .scroll((scroll_offset, 0));
                    f.render_widget(events_widget, center_chunks[1]);

                    // Footer
                    let footer = Paragraph::new("Press 'q' to quit")
                        .block(Block::default().borders(Borders::ALL));
                    f.render_widget(footer, chunks[2]);
                })?;
            }
            Ok(event_str) = rx.recv() => {
                state.events.push(event_str);
                if state.events.len() > 100 {
                    state.events.remove(0);
                }
            }
            event_res = tokio::task::spawn_blocking(|| event::read()) => {
                match event_res {
                    Ok(Ok(Event::Key(key))) => {
                        if key.code == KeyCode::Char('q') {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
