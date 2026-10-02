use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use lofty::file::TaggedFileExt;
use lofty::config::WriteOptions;
use lofty::picture::{Picture, PictureType};
use lofty::tag::{Accessor, Tag};
use lofty::prelude::*;
use reqwest::Client;
use rss::Channel;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Semaphore};
use crate::tui::download_ui::DownloadProgressRef;

#[derive(Clone, Debug)]
pub struct Episode {
    pub index: usize,
    pub id: String,
    pub title: String,
    pub url: String,
}

pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

pub fn load_archive(archive_path: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(file) = fs::File::open(archive_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                set.insert(trimmed.to_string());
            }
        }
    }
    set
}

pub async fn save_metadata_and_artwork(
    client: &Client,
    channel: &Channel,
    output_dir: &Path,
) -> Result<Option<(Vec<u8>, String)>> {
    fs::create_dir_all(output_dir)?;

    let title = &channel.title;
    let author = channel
        .itunes_ext()
        .and_then(|e| e.author())
        .unwrap_or("Unknown Author");
    let description = &channel.description;
    let link = channel.link();

    let image_url = channel
        .image()
        .map(|i| i.url().to_string())
        .or_else(|| {
            channel
                .itunes_ext()
                .and_then(|e| e.image())
                .map(|img| img.to_string())
        });

    let mut artwork_data = None;

    if let Some(url) = image_url {
        let ext = if url.contains(".png") { "png" } else { "jpg" };
        let mime = if ext == "png" { "image/png" } else { "image/jpeg" };

        let cover_path = output_dir.join(format!("cover.{}", ext));
        let folder_path = output_dir.join(format!("folder.{}", ext));

        if let Ok(res) = client.get(&url).send().await {
            if let Ok(bytes) = res.bytes().await {
                let vec_bytes = bytes.to_vec();
                if !cover_path.exists() {
                    let _ = fs::write(&cover_path, &vec_bytes);
                    let _ = fs::write(&folder_path, &vec_bytes);
                }
                artwork_data = Some((vec_bytes, mime.to_string()));
            }
        }
    }

    // Save metadata.nfo for Jellyfin / Audiobookshelf
    let nfo_path = output_dir.join("metadata.nfo");
    if !nfo_path.exists() {
        let nfo_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<audiobook>
  <title>{}</title>
  <author>{}</author>
  <publisher>{}</publisher>
  <plot>{}</plot>
  <description>{}</description>
  <link>{}</link>
</audiobook>
"#,
            quick_xml_escape(title),
            quick_xml_escape(author),
            quick_xml_escape(author),
            quick_xml_escape(description),
            quick_xml_escape(description),
            quick_xml_escape(link)
        );
        let _ = fs::write(&nfo_path, nfo_content);
    }

    Ok(artwork_data)
}

fn quick_xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Native tagger using `lofty` to write ID3/MP4 metadata and embed cover images
fn tag_audio_file(
    file_path: &Path,
    title: &str,
    artist: &str,
    album: &str,
    artwork: &Option<(Vec<u8>, String)>,
) -> Result<()> {
    let mut tagged_file = lofty::probe::Probe::open(file_path)?.read()?;

    // Use primary_tag_mut(), creating a new tag if none exists
    let tag = match tagged_file.primary_tag_mut() {
        Some(t) => t,
        None => {
            let tag_type = tagged_file.primary_tag_type();
            tagged_file.insert_tag(Tag::new(tag_type));
            tagged_file.primary_tag_mut().unwrap()
        }
    };

    tag.set_title(title.to_string());
    tag.set_artist(artist.to_string());
    tag.set_album(album.to_string());

    if let Some((bytes, mime)) = artwork {
        let picture = Picture::new_unchecked(
            PictureType::CoverFront,
            Some(lofty::picture::MimeType::from_str(mime)),
            None,
            bytes.clone(),
        );
        tag.push_picture(picture);
    }

    // `save_to_path` is now in scope via `use lofty::prelude::*;`
    tag.save_to_path(file_path, WriteOptions::default())?;
    Ok(())
}

pub async fn download_episodes(
    client: &Client,
    channel: &Channel,
    output_dir: &Path,
    max_concurrent: usize,
    progress: DownloadProgressRef,
) -> Result<()> {
    let artwork_info = save_metadata_and_artwork(client, channel, output_dir).await?;
    let podcast_title = channel.title.clone();
    let podcast_author = channel
        .itunes_ext()
        .and_then(|e| e.author())
        .unwrap_or("Unknown Author")
        .to_string();

    let archive_path = output_dir.join("downloaded_episodes.txt");
    let downloaded_ids = load_archive(&archive_path);

    let mut items: Vec<_> = channel.items().to_vec();
    items.reverse(); // Oldest to newest

    let mut pending: Vec<Episode> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        let id = item
            .guid()
            .map(|g| g.value().to_string())
            .or_else(|| item.enclosure().map(|e| e.url().to_string()))
            .or_else(|| item.title().map(|t| t.to_string()))
            .unwrap_or_else(|| format!("episode_{}", idx + 1));

        if !downloaded_ids.contains(&id) {
            let title = item.title().unwrap_or("Untitled Episode").to_string();
            let url = item
                .enclosure()
                .map(|e| e.url().to_string())
                .or_else(|| item.link().map(|l| l.to_string()))
                .ok_or_else(|| anyhow!("No media URL found for episode: {}", title))?;

            pending.push(Episode {
                index: idx + 1,
                id,
                title,
                url,
            });
        }
    }

    if pending.is_empty() {
        return Ok(());
    }

    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let archive_mutex = Arc::new(Mutex::new(archive_path));
    let artwork_info = Arc::new(artwork_info);
    let client = client.clone();
    let mut tasks = Vec::new();

    for ep in pending {
        let sem = Arc::clone(&semaphore);
        let archive_mutex = Arc::clone(&archive_mutex);
        let artwork_info = Arc::clone(&artwork_info);
        let progress = Arc::clone(&progress);
        let client = client.clone();
        let output_dir = output_dir.to_path_buf();
        let p_title = podcast_title.clone();
        let p_author = podcast_author.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            let truncated_title = if ep.title.len() > 50 {
                format!("{}...", &ep.title[..47])
            } else {
                ep.title.clone()
            };

            // Register this download in the progress tracker
            {
                let mut prog = progress.lock().await;
                prog.start_download(ep.index, truncated_title.clone());
            }

            // Determine extension from URL (defaults to mp3)
            let ext = if ep.url.contains(".m4a") {
                "m4a"
            } else if ep.url.contains(".aac") {
                "aac"
            } else if ep.url.contains(".ogg") {
                "ogg"
            } else {
                "mp3"
            };

            let filename = format!("{:02} - {}.{}", ep.index, sanitize_filename(&ep.title), ext);
            let file_path = output_dir.join(&filename);

            // Stream HTTP download directly via reqwest
            match client.get(&ep.url).send().await {
                Ok(response) => {
                    let total_size = response.content_length().unwrap_or(0);

                    if let Ok(mut file) = File::create(&file_path).await {
                        let mut stream = response.bytes_stream();
                        let mut downloaded: u64 = 0;
                        let mut success = true;

                        while let Some(chunk_res) = stream.next().await {
                            if let Ok(chunk) = chunk_res {
                                if file.write_all(&chunk).await.is_err() {
                                    success = false;
                                    break;
                                }
                                downloaded += chunk.len() as u64;

                                // Update progress display for this specific download
                                let mut prog = progress.lock().await;
                                prog.update_download(ep.index, downloaded, total_size);
                            } else {
                                success = false;
                                break;
                            }
                        }

                        let _ = file.flush().await;
                        drop(file);

                        if success {
                            // Tag metadata and embed cover art natively
                            let _ = tag_audio_file(
                                &file_path,
                                &ep.title,
                                &p_author,
                                &p_title,
                                &artwork_info,
                            );

                            let mut prog = progress.lock().await;
                            prog.mark_completed(ep.index, truncated_title.clone());

                            let lock = archive_mutex.lock().await;
                            if let Ok(mut archive_file) =
                                OpenOptions::new().create(true).append(true).open(&*lock)
                            {
                                let _ = writeln!(archive_file, "{}", ep.id);
                            }
                        } else {
                            let mut prog = progress.lock().await;
                            prog.mark_failed(ep.index, truncated_title.clone());
                        }
                    }
                }
                Err(_) => {
                    let mut prog = progress.lock().await;
                    prog.mark_failed(ep.index, truncated_title.clone());
                }
            }
        });

        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }

    Ok(())
}