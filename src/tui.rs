use anyhow::{Result, anyhow};
use chrono::DateTime;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};
use serde::Deserialize;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::analysis;
use crate::storage_utils::AsyncStorageManager;

// --- Data & App State ---

#[derive(Deserialize, Debug, Clone)]
pub struct OutputData {
    pub last_updated_timestamp: i64,
    pub results: Vec<AssetResult>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssetResult {
    pub symbol: String,
    #[serde(rename = "subType")]
    pub sub_type: Vec<String>,
    pub movement_pct: f64,
}

struct App {
    data: OutputData,
    is_refreshing: bool,
}

impl App {
    async fn new() -> Result<Self> {
        let initial_data = load_data().await.unwrap_or_else(|_| OutputData {
            last_updated_timestamp: 0,
            results: Vec::new(),
        });
        Ok(Self {
            data: initial_data,
            is_refreshing: false,
        })
    }

    fn set_data(&mut self, new_data: OutputData) {
        self.data = new_data;
        self.is_refreshing = false;
    }
}

// --- Data Loading ---

pub async fn load_data() -> Result<OutputData> {
    let storage = AsyncStorageManager::new_relative("storage").await?;
    storage.load("results").await
}

// --- TUI ---

pub async fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    res
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    let (data_tx, mut data_rx) = mpsc::channel::<Result<OutputData>>(1);
    let mut app = App::new().await?;

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Ok(result) = data_rx.try_recv() {
            match result {
                Ok(new_data) => app.set_data(new_data),
                Err(_) => {
                    app.is_refreshing = false;
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if !handle_key_event(key, &mut app, &data_tx) {
                    return Err(anyhow!("Quit"));
                }
            }
        }
    }
}

fn handle_key_event(key: KeyEvent, app: &mut App, tx: &mpsc::Sender<Result<OutputData>>) -> bool {
    match key.code {
        KeyCode::Char('q') => return false,
        KeyCode::F(5) if !app.is_refreshing => {
            app.is_refreshing = true;
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                let result = match analysis::run_analysis_pipeline().await {
                    Ok(_) => load_data().await,
                    Err(e) => Err(e),
                };
                let _ = tx_clone.send(result).await;
            });
        }
        _ => {}
    }
    true
}

fn ui(f: &mut Frame, app: &App) {
    let main_chunks = Layout::vertical([Constraint::Min(0)]).split(f.size());
    let top_chunks =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(main_chunks[0]);

    let time_str = format_timestamp(app.data.last_updated_timestamp);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title_alignment(Alignment::Center),
        top_chunks[0],
    );

    let header = Row::new(["Rank", "Asset", "Type", "Movement (%)"])
        .style(Style::default().bg(Color::DarkGray));
    let top_mover_pct = app.data.results.first().map_or(1.0, |r| r.movement_pct);
    let safe_top_pct = if top_mover_pct == 0.0 {
        1.0
    } else {
        top_mover_pct
    };

    let rows = app
        .data
        .results
        .iter()
        .take(100)
        .enumerate()
        .map(|(i, asset)| {
            let ratio = get_visibility_ratio(asset.movement_pct, safe_top_pct);
            let cyan_val = (255.0 * ratio) as u8;
            let green_val = (255.0 * ratio) as u8;
            let gray_val = (150.0 * ratio) as u8;
            let subtype_str = if asset.sub_type.is_empty() {
                "N/A".to_string()
            } else {
                format!("({})", asset.sub_type.join(", "))
            };

            Row::new([
                Cell::from(format!("{}", i + 1)).style(Style::default().fg(Color::DarkGray)),
                Cell::from(asset.symbol.clone())
                    .style(Style::default().fg(Color::Rgb(0, cyan_val, cyan_val))),
                Cell::from(subtype_str)
                    .style(Style::default().fg(Color::Rgb(gray_val, gray_val, gray_val))),
                Cell::from(
                    Line::from(format!("{:.2}%", asset.movement_pct)).alignment(Alignment::Right),
                )
                .style(Style::default().fg(Color::Rgb(0, green_val, 0))),
            ])
            .height(1)
        });
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(6),
                Constraint::Length(20),
                Constraint::Min(20),
                Constraint::Length(20),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Top Movers")),
        top_chunks[1],
    );

    if app.is_refreshing {
        let area = centered_rect(60, 20, f.size());
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new("Running analysis pipeline...\nPlease wait.")
                .block(Block::default().title("Refreshing").borders(Borders::ALL))
                .alignment(Alignment::Center),
            area,
        );
    }
}

fn get_visibility_ratio(current_pct: f64, top_pct: f64) -> f64 {
    if top_pct <= 0.0 {
        1.0
    } else {
        (0.4 + 0.6 * (current_pct / top_pct)).max(0.4)
    }
}

fn format_timestamp(ts_ms: i64) -> String {
    if ts_ms == 0 {
        return "Never".to_string();
    }
    let seconds = ts_ms / 1000;
    let nanoseconds = (ts_ms % 1000 * 1_000_000) as u32;
    DateTime::from_timestamp(seconds, nanoseconds)
        .map(|dt| dt.format("%d-%m-%Y %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown Time".to_string())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
