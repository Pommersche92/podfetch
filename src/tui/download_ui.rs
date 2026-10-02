use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use super::Terminal;

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub index: usize,
    pub title: String,
    pub progress: u64,
    pub total: u64,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub total_episodes: usize,
    pub completed: usize,
    pub failed: usize,
    pub active_downloads: HashMap<usize, DownloadItem>,
    pub completed_downloads: Vec<String>,
    pub failed_downloads: Vec<String>,
    pub scroll_active_downloads: usize,
    pub scroll_history: usize,
}

impl DownloadProgress {
    pub fn new(total_episodes: usize) -> Self {
        Self {
            total_episodes,
            completed: 0,
            failed: 0,
            active_downloads: HashMap::new(),
            completed_downloads: Vec::new(),
            failed_downloads: Vec::new(),
            scroll_active_downloads: 0,
            scroll_history: 0,
        }
    }

    /// Start tracking a download by episode index
    pub fn start_download(&mut self, index: usize, title: String) {
        self.active_downloads.insert(
            index,
            DownloadItem {
                index,
                title,
                progress: 0,
                total: 0,
            },
        );
    }

    /// Update progress for a specific download
    pub fn update_download(&mut self, index: usize, progress: u64, total: u64) {
        if let Some(item) = self.active_downloads.get_mut(&index) {
            item.progress = progress;
            item.total = total;
        }
    }

    /// Mark a download as completed and remove from active
    pub fn mark_completed(&mut self, index: usize, title: String) {
        self.active_downloads.remove(&index);
        self.completed += 1;
        self.completed_downloads.push(title);
    }

    /// Mark a download as failed and remove from active
    pub fn mark_failed(&mut self, index: usize, title: String) {
        self.active_downloads.remove(&index);
        self.failed += 1;
        self.failed_downloads.push(title);
    }

    /// Get sorted list of active downloads for display
    pub fn get_active_downloads(&self) -> Vec<DownloadItem> {
        let mut items: Vec<_> = self.active_downloads.values().cloned().collect();
        items.sort_by_key(|item| item.index);
        items
    }

    pub fn is_complete(&self) -> bool {
        self.completed + self.failed >= self.total_episodes
    }

    pub fn percent(&self) -> f64 {
        if self.total_episodes == 0 {
            0.0
        } else {
            ((self.completed + self.failed) as f64 / self.total_episodes as f64) * 100.0
        }
    }

    /// Scroll active downloads up
    pub fn scroll_active_up(&mut self) {
        if self.scroll_active_downloads > 0 {
            self.scroll_active_downloads -= 1;
        }
    }

    /// Scroll active downloads down
    pub fn scroll_active_down(&mut self, max_height: usize) {
        let total_items = self.active_downloads.len();
        let max_scroll = total_items.saturating_sub(1);
        if self.scroll_active_downloads < max_scroll && total_items > max_height {
            self.scroll_active_downloads += 1;
        }
    }

    /// Scroll history up
    pub fn scroll_history_up(&mut self) {
        if self.scroll_history > 0 {
            self.scroll_history -= 1;
        }
    }

    /// Scroll history down
    pub fn scroll_history_down(&mut self, max_height: usize) {
        let total_items = self.completed_downloads.len() + self.failed_downloads.len();
        let max_scroll = total_items.saturating_sub(1);
        if self.scroll_history < max_scroll && total_items > max_height {
            self.scroll_history += 1;
        }
    }
}

pub type DownloadProgressRef = Arc<Mutex<DownloadProgress>>;

pub async fn run_download_ui(
    terminal: &mut Terminal,
    progress: DownloadProgressRef,
) -> Result<()> {
    let mut focus_active = true; // Start with focus on active downloads

    loop {
        let prog = progress.lock().await;
        terminal.draw(|f| draw_download(f, &prog, focus_active))?;
        drop(prog);

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Ctrl-Q to cancel everything
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) 
                    && key.modifiers.contains(KeyModifiers::CONTROL) {
                    std::process::exit(1);
                }

                // Tab to switch focus between areas
                if matches!(key.code, KeyCode::Tab) {
                    focus_active = !focus_active;
                }

                // Up/Down to scroll
                if matches!(key.code, KeyCode::Up) {
                    let mut prog = progress.lock().await;
                    if focus_active {
                        prog.scroll_active_up();
                    } else {
                        prog.scroll_history_up();
                    }
                }

                if matches!(key.code, KeyCode::Down) {
                    let mut prog = progress.lock().await;
                    if focus_active {
                        prog.scroll_active_down(10); // Approximate max height
                    } else {
                        prog.scroll_history_down(10); // Approximate max height
                    }
                }
            }
        }

        let prog = progress.lock().await;
        if prog.is_complete() {
            break;
        }
    }

    Ok(())
}

fn draw_download(f: &mut Frame, progress: &DownloadProgress, focus_active: bool) {
    let area = f.area();

    // Header
    let header_layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ]);
    let [header_area, content_area, status_area] = header_layout.areas(area);

    let header_text = format!(
        "📥 Downloading Episodes: {}/{} ({:.1}%)",
        progress.completed + progress.failed,
        progress.total_episodes,
        progress.percent()
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, header_area);

    // Content layout
    let content_layout = Layout::vertical([
        Constraint::Min(10),
        Constraint::Length(3),
        Constraint::Min(5),
    ]);
    let [downloads_area, progress_bar_area, history_area] = content_layout.areas(content_area);

    // Current downloads
    draw_current_downloads(f, downloads_area, progress, focus_active);

    // Progress bar
    draw_progress_bar(f, progress_bar_area, progress);

    // Completed/Failed history
    draw_history(f, history_area, progress, !focus_active);

    // Status bar
    let status = Paragraph::new("Tab: Switch Focus | ↑↓: Scroll | Ctrl-Q: Cancel and Exit")
        .style(Style::default().fg(Color::DarkGray).italic())
        .alignment(Alignment::Center);
    f.render_widget(status, status_area);
}

fn draw_current_downloads(f: &mut Frame, area: Rect, progress: &DownloadProgress, focused: bool) {
    let active = progress.get_active_downloads();
    
    if active.is_empty() {
        let empty = Paragraph::new("Waiting for downloads to start...")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let title = Paragraph::new("Current Downloads")
        .style(Style::default().fg(Color::Green).bold());

    let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]);
    let [title_area, downloads_area] = layout.areas(area);
    f.render_widget(title, title_area);

    let max_display = downloads_area.height.saturating_sub(2) as usize; // Account for borders
    let scroll = progress.scroll_active_downloads;
    let visible_items: Vec<&DownloadItem> = active.iter().skip(scroll).take(max_display).collect();

    let downloads: Vec<Line> = visible_items
        .iter()
        .map(|item| {
            let percent = if item.total > 0 {
                (item.progress as f64 / item.total as f64) * 100.0
            } else {
                0.0
            };

            let bar_width: usize = 20;
            let filled = ((percent / 100.0) * bar_width as f64) as usize;
            let bar = format!(
                "[{}{}] {:.0}%",
                "█".repeat(filled),
                "░".repeat(bar_width.saturating_sub(filled)),
                percent
            );

            let truncated_title = if item.title.len() > 40 {
                format!("{}...", &item.title[..37])
            } else {
                item.title.clone()
            };

            Line::from(vec![
                Span::raw(format!("{:02}. ", item.index)),
                Span::styled(truncated_title.clone(), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(bar, Style::default().fg(Color::Cyan)),
            ])
        })
        .collect();

    let border_type = if focused {
        BorderType::Double
    } else {
        BorderType::Rounded
    };

    let border_color = if focused {
        Color::Cyan
    } else {
        Color::White
    };

    // Add scroll indicator
    let mut scroll_text = String::new();
    if scroll > 0 {
        scroll_text.push_str("▲ ");
    }
    if active.len() > scroll + max_display {
        scroll_text.push_str("▼");
    }

    let title_with_scroll = if !scroll_text.is_empty() {
        format!("Current Downloads  {}", scroll_text)
    } else {
        "Current Downloads".to_string()
    };

    let downloads_widget = Paragraph::new(downloads)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .title(title_with_scroll)
                .title_alignment(Alignment::Left)
                .border_style(Style::default().fg(border_color))
        );
    f.render_widget(downloads_widget, downloads_area);
}

fn draw_progress_bar(f: &mut Frame, area: Rect, progress: &DownloadProgress) {
    let percent = progress.percent();
    let bar_width = (area.width as f64 * percent / 100.0) as usize;

    let filled_bar = "█".repeat(bar_width);
    let empty_bar = "░".repeat(area.width.saturating_sub(bar_width as u16) as usize);

    let bar_text = format!("{}{} {:.1}%", filled_bar, empty_bar, percent);

    let progress_widget = Paragraph::new(bar_text)
        .style(if progress.failed > 0 {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            Style::default().bg(Color::Green).fg(Color::Black)
        })
        .alignment(Alignment::Center);

    f.render_widget(progress_widget, area);
}

fn draw_history(f: &mut Frame, area: Rect, progress: &DownloadProgress, focused: bool) {
    let title = Paragraph::new("History")
        .style(Style::default().fg(Color::Magenta).bold());

    let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(2)]);
    let [title_area, history_area] = layout.areas(area);
    f.render_widget(title, title_area);

    // Build all history items (most recent first)
    let mut all_items: Vec<(String, Color)> = Vec::new();
    
    for item in progress.completed_downloads.iter().rev() {
        let truncated = if item.len() > 45 {
            format!("{}...", &item[..42])
        } else {
            item.clone()
        };
        all_items.push((format!("✅ {}", truncated), Color::Green));
    }
    
    for item in progress.failed_downloads.iter().rev() {
        let truncated = if item.len() > 45 {
            format!("{}...", &item[..42])
        } else {
            item.clone()
        };
        all_items.push((format!("❌ {}", truncated), Color::Red));
    }

    // Apply scrolling
    let max_display = history_area.height.saturating_sub(2) as usize; // Account for borders
    let scroll = progress.scroll_history;
    let visible_items: Vec<&(String, Color)> = all_items.iter().skip(scroll).take(max_display).collect();

    let mut history_lines: Vec<Line> = visible_items
        .iter()
        .map(|(text, color)| {
            Line::from(text.clone())
                .style(Style::default().fg(*color))
        })
        .collect();

    if history_lines.is_empty() {
        history_lines.push(Line::from("No completed downloads yet"));
    }

    let border_type = if focused {
        BorderType::Double
    } else {
        BorderType::Rounded
    };

    let border_color = if focused {
        Color::Magenta
    } else {
        Color::White
    };

    // Add scroll indicator
    let mut scroll_text = String::new();
    if scroll > 0 {
        scroll_text.push_str("▲ ");
    }
    if all_items.len() > scroll + max_display {
        scroll_text.push_str("▼");
    }

    let title_with_scroll = if !scroll_text.is_empty() {
        format!("History  {}", scroll_text)
    } else {
        "History".to_string()
    };

    let history_widget = Paragraph::new(history_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .title(title_with_scroll)
                .title_alignment(Alignment::Left)
                .border_style(Style::default().fg(border_color))
        );
    f.render_widget(history_widget, history_area);
}
