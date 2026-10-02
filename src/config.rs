use anyhow::{Context, Result};
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_TEMPLATE: &str = "~/Downloads/podfetch/{podcast_name}";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub download_dir: String,
}

impl Config {
    pub fn load_or_init() -> Result<(Self, bool)> {
        // ~/.local/share/podfetch/config.toml
        let local_share = dirs::data_local_dir().context("Could not locate local share directory")?;
        let config_dir = local_share.join("podfetch");
        let config_file = config_dir.join("config.toml");

        // 1. Read existing config
        if config_file.exists() {
            let content = fs::read_to_string(&config_file)
                .context("Failed to read existing config file")?;
            let config: Config = toml::from_str(&content)
                .context("Failed to parse config.toml")?;
            return Ok((config, false)); // Not first run
        }

        // 2. FIRST RUN PROMPT - Very first action
        println!("============================================================");
        println!(" Welcome to PodFetch!");
        println!(" First time setup: Please configure your default download directory.");
        println!(" Available placeholders:");
        println!("   - {{podcast_name}}   (e.g., Darknet Diaries)");
        println!("   - {{podcast_author}} (e.g., Jack Rhysider)");
        println!(" Press Enter to use default: {}", DEFAULT_TEMPLATE);
        println!("============================================================\n");

        let input: String = Input::new()
            .with_prompt("Enter download directory template")
            .allow_empty(true)
            .interact_text()?;

        let chosen_path = input.trim();

        let final_template = if chosen_path.is_empty() {
            DEFAULT_TEMPLATE.to_string()
        } else {
            chosen_path.to_string()
        };

        // Create ~/.local/share/podfetch and write config.toml
        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        let config = Config {
            download_dir: final_template,
        };

        let toml_string = toml::to_string_pretty(&config)?;
        fs::write(&config_file, toml_string)?;

        println!("\nSaved configuration to: {}\n", config_file.display());

        Ok((config, true)) // First run
    }

    /// Dynamically expands placeholders ({podcast_name}, {podcast_author}, ~) into a real path
    pub fn resolve_path(&self, template_override: Option<&str>, podcast_name: &str, podcast_author: &str) -> PathBuf {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let raw_template = template_override.unwrap_or(&self.download_dir);

        let sanitized_name = sanitize_filename(podcast_name);
        let sanitized_author = sanitize_filename(podcast_author);

        // Replace placeholders
        let expanded_str = raw_template
            .replace("{podcast_name}", &sanitized_name)
            .replace("{podcast_author}", &sanitized_author);

        expand_tilde(Path::new(&expanded_str), &home_dir)
    }
}

/// Helper function to clean illegal path characters from podcast titles/authors
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Helper function to expand ~ to user's home directory
fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        home.join(stripped)
    } else {
        path.to_path_buf()
    }
}