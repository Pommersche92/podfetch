use anyhow::{anyhow, Context, Result};
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct PodcastSearchResult {
    pub title: String,
    pub author: String,
    pub page_or_feed_url: String,
    pub direct_rss: Option<String>,
}

#[derive(Deserialize)]
struct ITunesSearchResponse {
    results: Vec<ITunesItem>,
}

#[derive(Deserialize)]
struct ITunesItem {
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "feedUrl")]
    feed_url: Option<String>,
    #[serde(rename = "collectionViewUrl")]
    view_url: Option<String>,
}

/// Searches PodcastAddict for a podcast by query string.
/// Falls back to Apple/iTunes Podcast Index if web scraping is blocked or returns 0 results.
pub async fn search_podcasts(client: &reqwest::Client, query: &str) -> Result<Vec<PodcastSearchResult>> {
    let encoded_query = urlencoding::encode(query);
    
    // 1. Attempt PodcastAddict Search
    let pa_url = format!("https://podcastaddict.com/search?q={}", encoded_query);
    if let Ok(results) = search_podcastaddict_web(client, &pa_url).await {
        if !results.is_empty() {
            return Ok(results);
        }
    }

    // 2. Fallback: Query iTunes Podcast Search API (Indexed by PodcastAddict)
    let itunes_url = format!(
        "https://itunes.apple.com/search?term={}&entity=podcast&limit=10",
        encoded_query
    );
    search_itunes_api(client, &itunes_url).await
}

/// Scrapes PodcastAddict search result page
async fn search_podcastaddict_web(
    client: &reqwest::Client,
    search_url: &str,
) -> Result<Vec<PodcastSearchResult>> {
    let response = client
        .get(search_url)
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await?
        .text()
        .await?;

    let document = Html::parse_document(&response);
    let card_selector = Selector::parse("div.podcast-item, div.card, a[href*='/podcast/']").unwrap();
    let title_selector = Selector::parse(".title, .card-title, h4, h5").unwrap();
    let author_selector = Selector::parse(".author, .subtitle, .card-subtitle").unwrap();

    let mut results = Vec::new();

    for element in document.select(&card_selector) {
        let href = element
            .value()
            .attr("href")
            .or_else(|| {
                element
                    .select(&Selector::parse("a").unwrap())
                    .next()
                    .and_then(|a| a.value().attr("href"))
            });

        if let Some(path) = href {
            if path.contains("/podcast/") {
                let full_url = if path.starts_with("http") {
                    path.to_string()
                } else {
                    format!("https://podcastaddict.com{}", path)
                };

                let title = element
                    .select(&title_selector)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_else(|| "Unknown Title".to_string());

                let author = element
                    .select(&author_selector)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().to_string())
                    .unwrap_or_else(|| "Unknown Author".to_string());

                // Avoid duplicates
                if !results.iter().any(|r: &PodcastSearchResult| r.page_or_feed_url == full_url) {
                    results.push(PodcastSearchResult {
                        title,
                        author,
                        page_or_feed_url: full_url,
                        direct_rss: None,
                    });
                }
            }
        }
    }

    Ok(results)
}

/// Fallback search using iTunes API
async fn search_itunes_api(
    client: &reqwest::Client,
    api_url: &str,
) -> Result<Vec<PodcastSearchResult>> {
    let res: ITunesSearchResponse = client
        .get(api_url)
        .header(USER_AGENT, "podcast_sync/1.0")
        .send()
        .await?
        .json()
        .await?;

    let results = res
        .results
        .into_iter()
        .filter_map(|item| {
            let feed_url = item.feed_url?;
            Some(PodcastSearchResult {
                title: item.collection_name.unwrap_or_else(|| "Untitled".to_string()),
                author: item.artist_name.unwrap_or_else(|| "Unknown".to_string()),
                page_or_feed_url: item.view_url.unwrap_or_else(|| feed_url.clone()),
                direct_rss: Some(feed_url),
            })
        })
        .collect();

    Ok(results)
}

/// Given a PodcastAddict webpage URL, fetches the page and extracts the raw RSS feed URL
pub async fn resolve_rss_url(client: &reqwest::Client, result: &PodcastSearchResult) -> Result<String> {
    // If we already have the direct RSS (e.g. from iTunes API fallback)
    if let Some(ref rss) = result.direct_rss {
        return Ok(rss.clone());
    }

    let html_content = client
        .get(&result.page_or_feed_url)
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
        .context("Failed to fetch PodcastAddict show page")?
        .text()
        .await?;

    let document = Html::parse_document(&html_content);

    // Look for standard RSS link tags or PodcastAddict RSS buttons
    let rss_link_selector = Selector::parse(
        "a[href*='.xml'], a[href*='feed'], a[title*='RSS'], link[type='application/rss+xml']",
    )
    .unwrap();

    for element in document.select(&rss_link_selector) {
        if let Some(href) = element.value().attr("href") {
            if href.starts_with("http") && !href.contains("podcastaddict.com") {
                return Ok(href.to_string());
            }
        }
    }

    Err(anyhow!(
        "Could not extract a valid RSS feed URL from {}",
        result.page_or_feed_url
    ))
}