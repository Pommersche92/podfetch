mod config;
mod downloader;
mod search;
mod tui;
mod linux_desktop;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::Config;
use reqwest::Client;
use rss::Channel;
use tui::{setup_terminal, restore_terminal, run_wizard, run_download_ui};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(name = "podfetch")]
#[command(about = "Download and sync podcasts from RSS feeds or PodcastAddict searches")]
struct Args {
    /// RSS Feed URL OR Podcast Name (Optional - will prompt if omitted)
    #[arg(value_name = "URL_OR_NAME")]
    input: Option<String>,

    /// Force treating input as a search term
    #[arg(short, long)]
    search: bool,

    /// Override download path/template (supports {podcast_name} and {podcast_author})
    #[arg(short, long)]
    output: Option<String>,

    /// Maximum concurrent downloads (default: 5)
    #[arg(short, long, default_value_t = 5)]
    jobs: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load config or prompt user on first run
    let (config, is_first_run) = Config::load_or_init()?;
    
    // Install desktop integration on first run (Linux only)
    if is_first_run {
        if let Err(e) = linux_desktop::install_desktop_integration() {
            eprintln!("Warning: Failed to install desktop integration: {}", e);
            // Don't fail the entire application for this
        }
    }
    
    let args = Args::parse();

    // 2. Setup TUI
    let mut terminal = setup_terminal()?;

    // Provide wizard config if all CLI args are present
    let wizard_config = if args.input.is_some() {
        Some((
            args.input.clone().unwrap(),
            args.search,
            args.output.clone(),
            args.jobs,
        ))
    } else {
        None
    };

    let wizard_result = run_wizard(&mut terminal, wizard_config).await?;
    
    let client = Client::builder().build()?;

    // 3. Resolve Podcast Search or Direct URL
    let (_target_rss_url, channel) = if !wizard_result.force_search && is_url(&wizard_result.input) {
        terminal.draw(|f| {
            f.render_widget(
                ratatui::widgets::Paragraph::new("🔗 Fetching RSS metadata...")
                    .style(ratatui::prelude::Style::default().fg(ratatui::prelude::Color::Cyan)),
                f.area(),
            );
        })?;

        let bytes = client.get(&wizard_result.input).send().await?.bytes().await?;
        let channel = Channel::read_from(&bytes[..])?;

        (wizard_result.input, channel)
    } else {
        terminal.draw(|f| {
            f.render_widget(
                ratatui::widgets::Paragraph::new("🔍 Searching PodcastAddict...")
                    .style(ratatui::prelude::Style::default().fg(ratatui::prelude::Color::Cyan)),
                f.area(),
            );
        })?;

        let search_results = search::search_podcasts(&client, &wizard_result.input).await?;

        if search_results.is_empty() {
            restore_terminal()?;
            return Err(anyhow!("No podcasts found matching query: '{}'", wizard_result.input));
        }

        let selected = if search_results.len() == 1 {
            search_results[0].clone()
        } else {
            // Use first result for now (wizard handles multi-selection in UI version)
            search_results[0].clone()
        };

        let rss_url = search::resolve_rss_url(&client, &selected).await?;

        let bytes = client.get(&rss_url).send().await?.bytes().await?;
        let channel = Channel::read_from(&bytes[..])?;

        (rss_url, channel)
    };

    let podcast_name = channel.title.clone();
    let podcast_author = channel
        .itunes_ext()
        .and_then(|e| e.author())
        .unwrap_or("Unknown Author")
        .to_string();

    // 4. Resolve target directory using placeholders
    let output_dir = config.resolve_path(
        wizard_result.output_override.as_deref(),
        &podcast_name,
        &podcast_author,
    );

    // 5. Create progress tracker for download UI
    let progress = Arc::new(Mutex::new(
        tui::download_ui::DownloadProgress::new(channel.items().len()),
    ));

    // 6. Run downloads with TUI
    let progress_clone = Arc::clone(&progress);
    let download_task = tokio::spawn(async move {
        downloader::download_episodes(
            &client,
            &channel,
            &output_dir,
            wizard_result.max_jobs,
            progress_clone,
        )
        .await
    });

    // 7. Display download UI
    run_download_ui(&mut terminal, progress).await?;

    // Wait for download to complete
    download_task.await??;

    restore_terminal()?;
    println!("✅ Sync complete!");

    Ok(())
}

fn is_url(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
}