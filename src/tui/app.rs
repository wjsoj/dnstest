//! Interactive TUI application.
//!
//! This module provides the main TUI application for interactive
//! DNS testing operations.

use crate::dns::SpeedTestResult;
use crate::error::Result as ColorResult;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;
use tokio::time::sleep;

/// Main TUI Application.
///
/// Provides an interactive terminal interface for DNS testing,
/// including speed tests, pollution checks, and result display.
pub struct App {
    dns_servers: Vec<crate::dns::DnsServer>,
    results: Vec<SpeedTestResult>,
    pollution_results: Vec<crate::dns::PollutionResult>,
    current_view: AppView,
    sort_by_latency: bool,
    testing: bool,
    progress: f64,
    status_message: String,
    selected_server: Option<usize>,
}

/// Application views/states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppView {
    /// Main menu view
    Menu,
    /// Speed test in progress view
    SpeedTest,
    /// Pollution check in progress view
    PollutionCheck,
    /// Results display view
    Results,
    /// Help screen view
    Help,
}

impl App {
    /// Create a new application instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dns_servers: Vec::new(),
            results: Vec::new(),
            pollution_results: Vec::new(),
            current_view: AppView::Menu,
            sort_by_latency: false,
            testing: false,
            progress: 0.0,
            status_message: "Welcome to dnstest!".to_string(),
            selected_server: None,
        }
    }

    /// Set the DNS servers for the application.
    pub fn set_dns_servers(&mut self, servers: Vec<crate::dns::DnsServer>) {
        self.dns_servers = servers;
    }

    /// Run the TUI application.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal cannot be initialized or
    /// if there's an error during the event loop.
    pub async fn run(&mut self) -> ColorResult<()> {
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;

        // Load default DNS list
        if let Ok(lists) = crate::config::ConfigLoader::load_all() {
            self.dns_servers = crate::config::ConfigLoader::merge(lists).servers;
        }

        loop {
            terminal.draw(|f| self.draw(f))?;

            if let Ok(event) = self.read_event() {
                if !self.handle_event(event) {
                    break;
                }
            }

            if self.testing {
                sleep(Duration::from_millis(50)).await;
            }
        }

        Ok(())
    }

    /// Read an input event (non-blocking).
    fn read_event(&self) -> io::Result<crossterm::event::Event> {
        use crossterm::event;
        // Non-blocking read
        if event::poll(Duration::from_millis(10))? {
            event::read()
        } else {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "no event"))
        }
    }

    /// Handle input events.
    ///
    /// # Arguments
    ///
    /// * `event` - The input event to handle
    ///
    /// # Returns
    ///
    /// Returns `false` if the application should exit.
    fn handle_event(&mut self, event: crossterm::event::Event) -> bool {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        match event {
            // Ctrl+C to exit
            crossterm::event::Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => return false,

            // q to go back or quit
            crossterm::event::Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            }) => {
                if self.current_view == AppView::Menu {
                    return false;
                }
                self.current_view = AppView::Menu;
                return true;
            }

            crossterm::event::Event::Key(key) => match self.current_view {
                AppView::Menu => self.handle_menu_key(key),
                AppView::SpeedTest => self.handle_results_key(key),
                AppView::PollutionCheck => self.handle_results_key(key),
                AppView::Results => self.handle_results_key(key),
                AppView::Help => {
                    if matches!(
                        key.code,
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h')
                    ) {
                        self.current_view = AppView::Menu;
                    }
                }
            },

            _ => {}
        }

        true
    }

    /// Handle key events in the menu view.
    fn handle_menu_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('1') => {
                self.current_view = AppView::SpeedTest;
                self.start_speed_test();
            }
            KeyCode::Char('2') => {
                self.current_view = AppView::PollutionCheck;
                self.start_pollution_check();
            }
            KeyCode::Char('3') => {
                self.current_view = AppView::Help;
            }
            KeyCode::Char('s') => {
                self.sort_by_latency = !self.sort_by_latency;
                if !self.results.is_empty() {
                    self.sort_results();
                }
            }
            KeyCode::Char('q') => {}
            _ => {}
        }
    }

    /// Handle key events in results views.
    fn handle_results_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(s) = self.selected_server {
                    self.selected_server = Some(s.saturating_sub(1));
                } else {
                    self.selected_server = Some(0);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.results.len();
                if len > 0 {
                    self.selected_server = Some(
                        self.selected_server
                            .map(|s| (s + 1).min(len - 1))
                            .unwrap_or(0),
                    );
                }
            }
            KeyCode::Char('s') => {
                self.sort_by_latency = !self.sort_by_latency;
                self.sort_results();
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.current_view = AppView::Menu;
                self.testing = false;
            }
            _ => {}
        }
    }

    /// Sort results based on current settings.
    fn sort_results(&mut self) {
        if self.sort_by_latency {
            self.results.sort_by(|a, b| {
                let a_lat = a.latency_ms.unwrap_or(f64::MAX);
                let b_lat = b.latency_ms.unwrap_or(f64::MAX);
                a_lat.partial_cmp(&b_lat).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            self.results
                .sort_by(|a, b| a.server.name.cmp(&b.server.name));
        }
    }

    /// Start a speed test.
    fn start_speed_test(&mut self) {
        self.testing = true;
        self.progress = 0.0;
        self.results.clear();
        self.status_message = "Testing DNS servers...".to_string();
    }

    /// Start a pollution check.
    fn start_pollution_check(&mut self) {
        self.testing = true;
        self.progress = 0.0;
        self.pollution_results.clear();
        self.status_message = "Checking DNS pollution...".to_string();
    }

    /// Draw the UI based on current view.
    fn draw(&self, f: &mut Frame) {
        match self.current_view {
            AppView::Menu => self.draw_menu(f),
            AppView::SpeedTest | AppView::PollutionCheck => self.draw_testing(f),
            AppView::Results => self.draw_results(f),
            AppView::Help => self.draw_help(f),
        }
    }

    /// Draw the main menu.
    fn draw_menu(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(f.size());

        // Title
        let title = Paragraph::new(Text::from(vec![Line::from(vec![
            Span::raw("DNS"),
            Span::styled("test", Style::default().fg(Color::Cyan)),
        ])]))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title("dnstest - DNS测速与污染检测")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        );
        f.render_widget(title, chunks[0]);

        // Menu items
        let menu_items = vec![
            ListItem::new("1. DNS测速 (Speed Test)"),
            ListItem::new("2. DNS污染检测 (Pollution Check)"),
            ListItem::new("3. 帮助 (Help)"),
            ListItem::new(""),
            ListItem::new("快捷键:"),
            ListItem::new("  s - 切换排序"),
            ListItem::new("  q - 返回/退出"),
            ListItem::new("  Ctrl+C - 退出程序"),
        ];

        let menu = List::new(menu_items)
            .block(
                Block::default()
                    .title("菜单")
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::White));
        f.render_widget(menu, chunks[1]);

        // Status bar
        let status = Paragraph::new(self.status_message.as_str())
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(status, chunks[2]);
    }

    /// Draw the testing progress view.
    fn draw_testing(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(f.size());

        let title_text = if self.current_view == AppView::SpeedTest {
            "DNS测速中..."
        } else {
            "DNS污染检测中..."
        };

        let title = Paragraph::new(title_text)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().title(title_text));
        f.render_widget(title, chunks[0]);

        // Progress bar
        let gauge = Gauge::default()
            .block(Block::default().title("进度"))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(self.progress as u16);
        f.render_widget(gauge, chunks[1]);

        // Current server
        let current =
            Paragraph::new(self.status_message.clone()).block(Block::default().title("当前服务器"));
        f.render_widget(current, chunks[2]);

        // Status
        let status =
            Paragraph::new("按 'q' 返回菜单").style(Style::default().fg(Color::DarkGray));
        f.render_widget(status, chunks[3]);
    }

    /// Draw the results view.
    fn draw_results(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(20),
                Constraint::Length(3),
            ])
            .split(f.size());

        let title = if self.current_view == AppView::SpeedTest {
            "DNS测速结果"
        } else {
            "DNS污染检测结果"
        };

        let title_widget = Paragraph::new(title)
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().title(title));
        f.render_widget(title_widget, chunks[0]);

        // Results table
        let rows: Vec<Row> = self
            .results
            .iter()
            .enumerate()
            .map(|(idx, r)| {
                let latency = r
                    .latency_ms
                    .map(|l| format!("{l:.1} ms"))
                    .unwrap_or_else(|| "Timeout".to_string());
                let style = if r.success {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };

                Row::new(vec![
                    Cell::from(format!("{}", idx + 1)),
                    Cell::from(r.server.name.clone()),
                    Cell::from(r.server.ip.clone()).style(style),
                    Cell::from(latency),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(20),
                Constraint::Length(18),
                Constraint::Length(12),
            ],
        )
        .block(Block::default().border_type(BorderType::Rounded));

        f.render_widget(table, chunks[1]);

        // Status bar
        let status = Paragraph::new("按 'q' 返回菜单 | 's' 排序")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(status, chunks[2]);
    }

    /// Draw the help screen.
    fn draw_help(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(20),
                Constraint::Length(3),
            ])
            .split(f.size());

        let title = Paragraph::new("帮助")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().title("帮助"));
        f.render_widget(title, chunks[0]);

        let help_text = vec![
            "dnstest - DNS测速与污染检测工具",
            "",
            "功能:",
            "  1. DNS测速 - 使用ICMP Ping测试DNS服务器延迟",
            "  2. DNS污染检测 - 对比系统DNS与公共DNS解析结果",
            "",
            "快捷键:",
            "  1/2/3 - 选择菜单项",
            "  s - 切换排序",
            "  上/下 - 导航",
            "  q - 返回/退出",
            "  h - 帮助",
            "  Ctrl+C - 退出程序",
            "",
            "更多信息请访问: https://github.com/dnstest",
        ];

        let help = Paragraph::new(help_text.join("\n"))
            .block(Block::default().border_type(BorderType::Rounded))
            .wrap(ratatui::widgets::Wrap::default());
        f.render_widget(help, chunks[1]);

        let status =
            Paragraph::new("按 'q' 或 'Esc' 返回").style(Style::default().fg(Color::DarkGray));
        f.render_widget(status, chunks[2]);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
