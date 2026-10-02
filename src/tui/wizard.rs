use crate::search::PodcastSearchResult;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};
use std::time::Duration;
use super::Terminal;

#[derive(Debug, Clone)]
pub struct WizardConfig {
    pub input: String,
    pub force_search: bool,
    pub output_override: Option<String>,
    pub max_jobs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WizardStep {
    PodcastInput,
    SearchConfirm,
    SelectResult,
    OutputPath,
    ReviewComplete,
}

pub struct Wizard {
    step: WizardStep,
    input: String,
    search_results: Vec<PodcastSearchResult>,
    selected_result: usize,
    output: String,
    force_search: bool,
    max_jobs: usize,
}

impl Wizard {
    pub fn new() -> Self {
        Self {
            step: WizardStep::PodcastInput,
            input: String::new(),
            search_results: Vec::new(),
            selected_result: 0,
            output: String::new(),
            force_search: false,
            max_jobs: 5,
        }
    }

    pub fn config(self) -> WizardConfig {
        WizardConfig {
            input: self.input,
            force_search: self.force_search,
            output_override: if self.output.is_empty() {
                None
            } else {
                Some(self.output)
            },
            max_jobs: self.max_jobs,
        }
    }
}

pub async fn run_wizard(terminal: &mut Terminal, args_config: Option<(String, bool, Option<String>, usize)>) -> Result<WizardConfig> {
    // If all args are provided, skip wizard
    if let Some((input, search, output, jobs)) = args_config {
        return Ok(WizardConfig {
            input,
            force_search: search,
            output_override: output,
            max_jobs: jobs,
        });
    }

    let mut wizard = Wizard::new();

    loop {
        terminal.draw(|f| draw_wizard(f, &wizard))?;

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if handle_wizard_input(&mut wizard, key)? {
                    break;
                }
            }
        }
    }

    Ok(wizard.config())
}

fn draw_wizard(f: &mut Frame, wizard: &Wizard) {
    let area = f.area();
    
    // Create layout with status bar at bottom
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(3),
        Constraint::Length(1),
    ]);
    let [header_area, content_area, footer_area, status_area] = vertical.areas(area);

    // Header
    let header = Paragraph::new("🎙️  Podcast Downloader Setup Wizard")
        .style(Style::default().fg(Color::Cyan).bold())
        .alignment(Alignment::Center);
    f.render_widget(header, header_area);

    // Content based on step
    match wizard.step {
        WizardStep::PodcastInput => {
            draw_podcast_input(f, content_area, wizard);
        }
        WizardStep::SearchConfirm => {
            draw_search_confirm(f, content_area, wizard);
        }
        WizardStep::SelectResult => {
            draw_select_result(f, content_area, wizard);
        }
        WizardStep::OutputPath => {
            draw_output_path(f, content_area, wizard);
        }
        WizardStep::ReviewComplete => {
            draw_review(f, content_area, wizard);
        }
    }

    // Footer with instructions
    let instructions = match wizard.step {
        WizardStep::PodcastInput => "Type podcast name or RSS URL, then press Enter",
        WizardStep::SearchConfirm => "[Y]es search / [N]o treat as URL / [B]ack",
        WizardStep::SelectResult => "↑↓ Navigate / Enter to select / [B]ack",
        WizardStep::OutputPath => "[Enter] use default / [C]ustom path / [B]ack",
        WizardStep::ReviewComplete => "Press [Enter] to start download",
    };
    
    let footer = Paragraph::new(instructions)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(footer, footer_area);

    // Status bar
    let status = Paragraph::new("Ctrl-Q: Cancel and Exit")
        .style(Style::default().fg(Color::DarkGray).italic())
        .alignment(Alignment::Center);
    f.render_widget(status, status_area);
}

fn draw_podcast_input(f: &mut Frame, area: Rect, wizard: &Wizard) {
    let input_prompt = Paragraph::new("Enter podcast name or RSS feed URL:")
        .style(Style::default().fg(Color::White));

    let input_box = Paragraph::new(format!("▶ {}_", wizard.input))
        .style(Style::default().fg(Color::Yellow).bold())
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));

    let vertical = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]);
    let [prompt_area, input_area] = vertical.areas(area);

    f.render_widget(input_prompt, prompt_area);
    f.render_widget(input_box, input_area);
}

fn draw_search_confirm(f: &mut Frame, area: Rect, wizard: &Wizard) {
    let mut lines = vec![
        Line::from("Is this a podcast name or description you want to search for?"),
        Line::from(""),
        Line::from(format!("▶ {}", wizard.input)).style(Style::default().fg(Color::Yellow)),
        Line::from(""),
    ];

    if wizard.force_search {
        lines.push(Line::from("(Treating as search term)").style(Style::default().fg(Color::Green)));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn draw_select_result(f: &mut Frame, area: Rect, wizard: &Wizard) {
    let header = Paragraph::new("Select a podcast from search results:")
        .style(Style::default().fg(Color::Cyan));

    let results: Vec<Line> = wizard
        .search_results
        .iter()
        .enumerate()
        .map(|(idx, result)| {
            let prefix = if idx == wizard.selected_result {
                "❯ "
            } else {
                "  "
            };
            let style = if idx == wizard.selected_result {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(format!("{}{} - {}", prefix, result.title, result.author))
                .style(style)
        })
        .collect();

    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]);
    let [header_area, results_area] = vertical.areas(area);

    f.render_widget(header, header_area);
    
    let results_list = Paragraph::new(results)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(results_list, results_area);
}

fn draw_output_path(f: &mut Frame, area: Rect, wizard: &Wizard) {
    let lines = vec![
        Line::from("Download location:"),
        Line::from(""),
        Line::from("Supports placeholders:").style(Style::default().fg(Color::Gray)),
        Line::from("  • {podcast_name}").style(Style::default().fg(Color::Gray)),
        Line::from("  • {podcast_author}").style(Style::default().fg(Color::Gray)),
        Line::from(""),
        Line::from(format!("Current: {}", if wizard.output.is_empty() { "[default]" } else { &wizard.output }))
            .style(Style::default().fg(Color::Yellow)),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));

    f.render_widget(paragraph, area);
}

fn draw_review(f: &mut Frame, area: Rect, wizard: &Wizard) {
    let lines = vec![
        Line::from("✓ Configuration Complete").style(Style::default().fg(Color::Green).bold()),
        Line::from(""),
        Line::from(format!("Input: {}", wizard.input)),
        Line::from(format!("Type: {}", if wizard.force_search { "Search" } else { "URL" })),
        Line::from(format!("Output: {}", if wizard.output.is_empty() { "[default]" } else { &wizard.output })),
        Line::from(format!("Max concurrent jobs: {}", wizard.max_jobs)),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn handle_wizard_input(wizard: &mut Wizard, key: KeyEvent) -> Result<bool> {
    // Ctrl-Q to exit
    if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) 
        && key.modifiers.contains(KeyModifiers::CONTROL) {
        std::process::exit(0);
    }

    // Esc to exit
    if matches!(key.code, KeyCode::Esc) {
        std::process::exit(0);
    }

    match key.code {
        KeyCode::Enter => {
            match wizard.step {
                WizardStep::PodcastInput => {
                    if !wizard.input.trim().is_empty() {
                        wizard.step = WizardStep::SearchConfirm;
                    }
                }
                WizardStep::SearchConfirm => {
                    // Stays in this step until y/n pressed
                }
                WizardStep::SelectResult => {
                    if !wizard.search_results.is_empty() {
                        wizard.step = WizardStep::OutputPath;
                    }
                }
                WizardStep::OutputPath => {
                    wizard.step = WizardStep::ReviewComplete;
                }
                WizardStep::ReviewComplete => {
                    return Ok(true); // Exit wizard
                }
            }
        }

        KeyCode::Backspace => {
            if matches!(wizard.step, WizardStep::PodcastInput) {
                wizard.input.pop();
            }
        }

        KeyCode::Up => {
            if matches!(wizard.step, WizardStep::SelectResult) {
                if wizard.selected_result > 0 {
                    wizard.selected_result -= 1;
                }
            }
        }

        KeyCode::Down => {
            if matches!(wizard.step, WizardStep::SelectResult) {
                if wizard.selected_result < wizard.search_results.len().saturating_sub(1) {
                    wizard.selected_result += 1;
                }
            }
        }

        // Specific character patterns MUST come before generic KeyCode::Char(c)
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if matches!(wizard.step, WizardStep::SearchConfirm) {
                wizard.force_search = true;
                wizard.step = WizardStep::SelectResult;
            }
        }

        KeyCode::Char('n') | KeyCode::Char('N') => {
            if matches!(wizard.step, WizardStep::SearchConfirm) {
                wizard.force_search = false;
                wizard.step = WizardStep::OutputPath;
            }
        }

        KeyCode::Char('b') | KeyCode::Char('B') => {
            match wizard.step {
                WizardStep::SearchConfirm => wizard.step = WizardStep::PodcastInput,
                WizardStep::SelectResult => wizard.step = WizardStep::SearchConfirm,
                WizardStep::OutputPath => wizard.step = WizardStep::SearchConfirm,
                WizardStep::ReviewComplete => wizard.step = WizardStep::OutputPath,
                _ => {}
            }
        }

        // Generic character input for podcast name (AFTER specific patterns)
        KeyCode::Char(c) => {
            if matches!(wizard.step, WizardStep::PodcastInput) {
                wizard.input.push(c);
            }
        }

        _ => {}
    }

    Ok(false)
}
