//! Interactive TUI application.

#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::manual_let_else)]

use crate::dns::{DnsServer, PollutionResult, SpeedTestResult};
use crate::error::Result as ColorResult;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Cell, Gauge, Paragraph, Row, Table, TableState},
    Frame,
};
use tokio::sync::mpsc;
use tokio::time::Duration;

/// Messages sent from async tasks to the main event loop.
#[derive(Debug)]
#[allow(dead_code)]
enum AppMessage {
    /// A single speed test result.
    Result(SpeedTestResult),
    /// Progress update.
    Progress { tested: usize, total: usize },
    /// All tests completed.
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Latency,
    Name,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum View {
    #[default]
    SpeedTest,
    PollutionCheck,
    Help,
}

pub struct App {
    dns_servers: Vec<DnsServer>,
    results: Vec<SpeedTestResult>,
    #[allow(dead_code)]
    pollution_results: Vec<(String, PollutionResult)>,
    current_view: View,
    tab_index: usize,
    sort_mode: SortMode,
    testing: bool,
    tested_count: usize,
    total_count: usize,
    selected_index: usize,
    /// Channel sender for async tasks.
    message_tx: Option<mpsc::UnboundedSender<AppMessage>>,
    /// Table state for scrolling.
    table_state: TableState,
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        Self {
            dns_servers: Vec::new(),
            results: Vec::new(),
            pollution_results: Vec::new(),
            current_view: View::default(),
            tab_index: 0,
            sort_mode: SortMode::Latency,
            testing: false,
            tested_count: 0,
            total_count: 0,
            selected_index: 0,
            message_tx: None,
            table_state: TableState::default(),
        }
    }

    pub fn set_dns_servers(&mut self, servers: Vec<DnsServer>) {
        self.dns_servers = servers;
    }

    pub async fn run(&mut self) -> ColorResult<()> {
        // Create channel for async task communication
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.message_tx = Some(tx);

        // Initialize terminal with raw mode and alternate screen
        let mut terminal = ratatui::init();

        // Load DNS server list
        if let Ok(lists) = crate::config::ConfigLoader::load_all() {
            let merged = crate::config::ConfigLoader::merge(lists);
            self.dns_servers = merged.servers;
        }
        self.total_count = self.dns_servers.len();

        let res = self.run_loop(&mut terminal, &mut rx).await;

        // Restore terminal state
        ratatui::restore();

        res
    }

    async fn run_loop(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        rx: &mut mpsc::UnboundedReceiver<AppMessage>,
    ) -> ColorResult<()> {
        loop {
            // 1. Process all pending messages from async tasks
            while let Ok(msg) = rx.try_recv() {
                self.handle_message(msg);
            }

            // 2. Render UI
            terminal.draw(|f| self.draw(f))?;

            // 3. Handle keyboard events (non-blocking with 50ms timeout)
            if crossterm::event::poll(Duration::from_millis(50))? {
                if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                    if !self.handle_key(key) {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::Result(result) => {
                self.results.push(result);
                self.tested_count += 1;
                // Real-time sorting during test
                self.sort_results();
            }
            AppMessage::Progress { tested, .. } => {
                self.tested_count = tested;
            }
            AppMessage::Completed => {
                self.testing = false;
                // Final sort
                self.sort_results();
            }
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return false;
            }

            KeyCode::Tab => {
                self.tab_index = (self.tab_index + 1) % 3;
                self.current_view = match self.tab_index {
                    0 => View::SpeedTest,
                    1 => View::PollutionCheck,
                    _ => View::Help,
                };
                return true;
            }

            KeyCode::Char('1') => {
                self.tab_index = 0;
                self.current_view = View::SpeedTest;
                return true;
            }
            KeyCode::Char('2') => {
                self.tab_index = 1;
                self.current_view = View::PollutionCheck;
                return true;
            }
            KeyCode::Char('3') => {
                self.tab_index = 2;
                self.current_view = View::Help;
                return true;
            }

            KeyCode::Char(' ') if self.current_view == View::SpeedTest => {
                if !self.testing {
                    self.start_speed_test();
                }
                return true;
            }

            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.table_state.select(Some(self.selected_index));
                }
                return true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.results.len().saturating_sub(1);
                if self.selected_index < max {
                    self.selected_index += 1;
                    self.table_state.select(Some(self.selected_index));
                }
                return true;
            }

            KeyCode::Char('s') if self.current_view == View::SpeedTest => {
                self.sort_mode = match self.sort_mode {
                    SortMode::Latency => SortMode::Name,
                    SortMode::Name => SortMode::Status,
                    SortMode::Status => SortMode::Latency,
                };
                self.sort_results();
                return true;
            }

            KeyCode::Char('q') if self.current_view != View::Help => {
                self.testing = false;
                return false;
            }

            KeyCode::Esc | KeyCode::Char('q') if self.current_view == View::Help => {
                self.tab_index = 0;
                self.current_view = View::SpeedTest;
                return true;
            }

            _ => {}
        }

        true
    }

    fn start_speed_test(&mut self) {
        self.testing = true;
        self.results.clear();
        self.tested_count = 0;
        self.selected_index = 0;

        let servers: Vec<DnsServer> = self.dns_servers.clone();
        self.total_count = servers.len();

        let Some(tx) = self.message_tx.clone() else {
            self.testing = false;
            return;
        };

        let total = servers.len();

        // Spawn async speed test task
        tokio::spawn(async move {
            use tokio::sync::Semaphore;

            const MAX_CONCURRENT: usize = 20;
            const TOTAL_TIMEOUT_SECS: u64 = 120;

            let semaphore = std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT));
            let tested = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

            let mut handles = Vec::new();

            for server in servers {
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let tx = tx.clone();
                let tested = tested.clone();

                let handle = tokio::spawn(async move {
                    let tester = match crate::dns::SpeedTester::new() {
                        Ok(t) => t,
                        Err(_) => {
                            drop(permit);
                            return;
                        }
                    };

                    let result = tester.test_latency(&server).await;
                    let count = tested.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                    // Send result and progress
                    let _ = tx.send(AppMessage::Result(result));
                    let _ = tx.send(AppMessage::Progress {
                        tested: count,
                        total,
                    });

                    drop(permit);
                });

                handles.push(handle);
            }

            // Wait for all tasks with timeout
            let timeout_result = tokio::time::timeout(
                Duration::from_secs(TOTAL_TIMEOUT_SECS),
                futures::future::join_all(handles),
            )
            .await;

            if timeout_result.is_err() {
                tracing::warn!("Speed test timed out");
            }

            // Signal completion
            let _ = tx.send(AppMessage::Completed);
        });
    }

    fn sort_results(&mut self) {
        match self.sort_mode {
            SortMode::Latency => {
                self.results.sort_by(|a, b| {
                    let a_lat = a.latency_ms.unwrap_or(f64::MAX);
                    let b_lat = b.latency_ms.unwrap_or(f64::MAX);
                    a_lat
                        .partial_cmp(&b_lat)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortMode::Name => {
                self.results
                    .sort_by(|a, b| a.server.name.cmp(&b.server.name));
            }
            SortMode::Status => {
                self.results.sort_by(|a, b| {
                    let a_order = if a.success { 0 } else { 1 };
                    let b_order = if b.success { 0 } else { 1 };
                    a_order.cmp(&b_order)
                });
            }
        }
    }

    fn get_stats(
        &self,
    ) -> (
        usize,
        usize,
        usize,
        usize,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    ) {
        let total = self.results.len();
        let success = self.results.iter().filter(|r| r.success).count();
        let timeout = self.results.iter().filter(|r| r.is_timeout()).count();
        let failed = total.saturating_sub(success).saturating_sub(timeout);

        let latencies: Vec<f64> = self.results.iter().filter_map(|r| r.latency_ms).collect();

        let avg = if latencies.is_empty() {
            None
        } else {
            Some(latencies.iter().sum::<f64>() / latencies.len() as f64)
        };

        let min = latencies.iter().copied().reduce(f64::min);
        let max = latencies.iter().copied().reduce(f64::max);

        (total, success, failed, timeout, avg, min, max)
    }

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(2),
                Constraint::Min(10),
                Constraint::Length(6),
            ])
            .split(f.area());

        self.draw_title_bar(f, chunks[0]);
        self.draw_tabs(f, chunks[1]);

        match self.current_view {
            View::SpeedTest => self.draw_speed_test(f, chunks[2]),
            View::PollutionCheck => self.draw_pollution_check(f, chunks[2]),
            View::Help => self.draw_help(f, chunks[2]),
        }

        self.draw_stats_bar(f, chunks[3]);
    }

    fn draw_title_bar(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20),
                Constraint::Min(10),
                Constraint::Length(20),
            ])
            .split(area);

        let title = Paragraph::new("DNS Speed Test").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(title, chunks[0]);

        let version = Paragraph::new("dnstest v0.1.0")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(version, chunks[1]);

        let server_count = Paragraph::new(format!("{} servers", self.dns_servers.len()))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Right);
        f.render_widget(server_count, chunks[2]);
    }

    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let titles = ["Speed Test", "Pollution", "Help"];
        let mut tab_text = String::new();
        for (i, title) in titles.iter().enumerate() {
            if i == self.tab_index {
                tab_text.push_str(&format!("[{}] ", title));
            } else {
                tab_text.push_str(&format!(" {} ", title));
            }
        }
        let tabs = Paragraph::new(tab_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().border_type(BorderType::Plain));
        f.render_widget(tabs, area);
    }

    fn draw_speed_test(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(10)])
            .split(area);

        let sort_indicator = match self.sort_mode {
            SortMode::Latency => "Latency",
            SortMode::Name => "Name",
            SortMode::Status => "Status",
        };
        let status_text = if self.testing {
            format!(
                "Testing... ({}/{}) | Sort by: {} [s]",
                self.tested_count, self.total_count, sort_indicator
            )
        } else {
            format!("Sort by: {} [s]", sort_indicator)
        };
        let header = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));
        f.render_widget(header, chunks[0]);

        if self.results.is_empty() {
            let msg = if self.testing {
                "Starting speed test..."
            } else {
                "Press [Space] to start speed test"
            };
            let empty_msg = Paragraph::new(msg)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(empty_msg, chunks[1]);
            return;
        }

        let rows: Vec<Row> = self
            .results
            .iter()
            .enumerate()
            .map(|(idx, r)| {
                let latency_bar = r.latency_ms.map_or_else(String::new, |l| {
                    let bar_len = ((l / 200.0) * 20.0).min(20.0) as usize;
                    "█".repeat(bar_len)
                });

                let latency_text = r
                    .latency_ms
                    .map_or_else(|| "Timeout".to_string(), |l| format!("{:.1}ms", l));

                let latency_style = if r.success {
                    Style::default().fg(Color::Green)
                } else if r.is_timeout() {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                };

                let selected = if idx == self.selected_index {
                    Style::default().bg(Color::Blue)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(format!("{}", idx + 1)).style(selected),
                    Cell::from(r.server.name.clone()).style(selected),
                    Cell::from(r.server.ip.clone()).style(selected),
                    Cell::from(latency_bar).style(latency_style),
                    Cell::from(latency_text).style(latency_style),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(25),
                Constraint::Length(18),
                Constraint::Length(22),
                Constraint::Length(12),
            ],
        )
        .block(Block::default().border_type(BorderType::Rounded))
        .row_highlight_style(Style::default().bg(Color::Blue));

        // Use stateful rendering for scroll support
        f.render_stateful_widget(table, chunks[1], &mut self.table_state);
    }

    fn draw_pollution_check(&self, f: &mut Frame, area: Rect) {
        let msg = Paragraph::new("Pollution check feature coming soon...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(msg, area);
    }

    fn draw_help(&self, f: &mut Frame, area: Rect) {
        use ratatui::widgets::{Clear, Wrap};

        // Clear the area first
        f.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Title
                Constraint::Min(1),    // Content
                Constraint::Length(2), // Footer
            ])
            .split(area);

        // Title
        let title = Paragraph::new("dnstest - Help")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(title, chunks[0]);

        // Help content using a table-like layout
        let help_items = [
            ("Space", "Start speed test"),
            ("s", "Cycle sort mode (Latency/Name/Status)"),
            ("j/k or Up/Down", "Navigate results"),
            ("1/2/3", "Switch tabs (Speed/Pollution/Help)"),
            ("Tab", "Cycle through tabs"),
            ("q", "Quit application"),
        ];

        let rows: Vec<Row> = help_items
            .iter()
            .map(|(key, desc)| {
                Row::new(vec![
                    Cell::from(format!("  {}  ", key)).style(Style::default().fg(Color::Yellow)),
                    Cell::from(*desc).style(Style::default().fg(Color::White)),
                ])
            })
            .collect();

        let help_table = Table::new(rows, [Constraint::Length(16), Constraint::Min(30)])
            .block(
                Block::default()
                    .title(" Keyboard Shortcuts ")
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(2);

        f.render_widget(help_table, chunks[1]);

        // Footer
        let footer = Paragraph::new("Press [q] or [Esc] to return to Speed Test")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(footer, chunks[2]);
    }

    fn draw_stats_bar(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3)])
            .split(area);

        let (total, success, failed, timeout, avg, min, max) = self.get_stats();

        let mut stats_parts = vec![format!("Total: {}", total), format!("Success: {}", success)];

        if failed > 0 {
            stats_parts.push(format!("Failed: {}", failed));
        }
        if timeout > 0 {
            stats_parts.push(format!("Timeout: {}", timeout));
        }
        if let Some(avg_lat) = avg {
            stats_parts.push(format!("Avg: {:.1}ms", avg_lat));
        }
        if let Some(min_lat) = min {
            stats_parts.push(format!("Min: {:.1}ms", min_lat));
        }
        if let Some(max_lat) = max {
            stats_parts.push(format!("Max: {:.1}ms", max_lat));
        }

        let stats_text = stats_parts.join("  |  ");

        let stats = Paragraph::new(stats_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(" Statistics ")
                    .border_type(BorderType::Rounded),
            );
        f.render_widget(stats, chunks[0]);

        let progress = if self.total_count > 0 {
            ((self.tested_count as f64 / self.total_count as f64) * 100.0).min(100.0) as u16
        } else {
            0
        };

        let progress_text = format!("{}/{} ({}%)", self.tested_count, self.total_count, progress);

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(progress_text)
                    .border_type(BorderType::Rounded),
            )
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(progress);

        f.render_widget(gauge, chunks[1]);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
