use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
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
        println!("🎉 All episodes are up to date!");
        return Ok(());
    }

    println!(
        "\n📥 Downloading {} new episode(s) natively (max {} parallel)...",
        pending.len(),
        max_concurrent
    );

    let multi = Arc::new(MultiProgress::new());
    let main_pb = multi.add(ProgressBar::new(pending.len() as u64));
    main_pb.set_style(
        ProgressStyle::with_template(
            "[{pos}/{len}] [{bar:30.cyan/blue}] {percent}% - Total Progress",
        )?
        .progress_chars("#>-"),
    );

    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let archive_mutex = Arc::new(Mutex::new(archive_path));
    let artwork_info = Arc::new(artwork_info);
    let client = client.clone();
    let mut tasks = Vec::new();

    for ep in pending {
        let sem = Arc::clone(&semaphore);
        let multi_clone = Arc::clone(&multi);
        let archive_mutex = Arc::clone(&archive_mutex);
        let artwork_info = Arc::clone(&artwork_info);
        let main_pb = main_pb.clone();
        let client = client.clone();
        let output_dir = output_dir.to_path_buf();
        let p_title = podcast_title.clone();
        let p_author = podcast_author.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            let pb = multi_clone.add(ProgressBar::new(100));
            pb.set_style(
                ProgressStyle::with_template(" [{bar:20.green/white}] {percent:>3}% | {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );

            let truncated_title = if ep.title.len() > 30 {
                format!("{}...", &ep.title[..27])
            } else {
                ep.title.clone()
            };

            pb.set_message(truncated_title.clone());

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
                    if let Some(content_length) = response.content_length() {
                        pb.set_length(content_length);
                    }

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
                                pb.set_position(downloaded);
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

                            pb.finish_with_message(format!("DONE: {}", truncated_title));

                            let lock = archive_mutex.lock().await;
                            if let Ok(mut archive_file) =
                                OpenOptions::new().create(true).append(true).open(&*lock)
                            {
                                let _ = writeln!(archive_file, "{}", ep.id);
                            }
                        } else {
                            pb.abandon_with_message(format!("FAILED: {}", truncated_title));
                        }
                    }
                }
                Err(_) => {
                    pb.abandon_with_message(format!("FAILED: {}", truncated_title));
                }
            }

            main_pb.inc(1);
        });

        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }

    main_pb.finish_with_message("Sync complete!");
    Ok(())
}