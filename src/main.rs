mod config;
mod downloader;
mod search;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::Config;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use reqwest::Client;
use rss::Channel;

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
    // 1. FIRST ACTION: Load config or prompt user on first run
    let config = Config::load_or_init()?;

    let args = Args::parse();
    let client = Client::builder().build()?;

    // 2. Resolve input: Use CLI argument or prompt the user if omitted
    let input = match args.input {
        Some(val) if !val.trim().is_empty() => val.trim().to_string(),
        _ => Input::<String>::new()
            .with_prompt("Enter podcast name or RSS feed URL")
            .interact_text()?,
    };

    // 3. Resolve Podcast Search or Direct URL
    let (target_rss_url, channel) = if !args.search && is_url(&input) {
        println!("🔗 Fetching RSS metadata from: {}", input);

        let bytes = client.get(&input).send().await?.bytes().await?;
        let channel = Channel::read_from(&bytes[..])?;

        (input, channel)
    } else {
        println!("🔍 Searching PodcastAddict for: \"{}\"...", input);
        let search_results = search::search_podcasts(&client, &input).await?;

        if search_results.is_empty() {
            return Err(anyhow!("No podcasts found matching query: '{}'", input));
        }

        let selected = if search_results.len() == 1 {
            println!("✅ Match found: \"{}\" by {}", search_results[0].title, search_results[0].author);
            search_results[0].clone()
        } else {
            let items: Vec<String> = search_results
                .iter()
                .map(|r| format!("{} (by {})", r.title, r.author))
                .collect();

            println!("\nMultiple podcasts found. Select one to proceed:");
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Choose a podcast")
                .default(0)
                .items(&items)
                .interact()?;

            search_results[selection].clone()
        };

        println!("📡 Resolving RSS feed for \"{}\"...", selected.title);
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
    let output_dir = config.resolve_path(args.output.as_deref(), &podcast_name, &podcast_author);

    println!("\n============================================================");
    println!("Podcast:     {}", podcast_name);
    println!("Author:      {}", podcast_author);
    println!("RSS URL:     {}", target_rss_url);
    println!("Output Dir:  {}", output_dir.display());
    println!("============================================================\n");

    // 5. Execute parallel downloading
    downloader::download_episodes(&client, &channel, &output_dir, args.jobs).await?;

    Ok(())
}

fn is_url(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
}