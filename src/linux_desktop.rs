// SPDX-FileCopyrightText: 2026 Raimo Geisel
// SPDX-License-Identifier: GPL-3.0

//! Linux desktop environment integration for AppImage.
//! 
//! Installs .desktop file and icon to standard XDG locations on first run
//! so that the application appears in application menus and launchers.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Install .desktop file and icon to standard XDG locations
/// 
/// This should only be called on first run (when config is created).
/// Does nothing on non-Linux systems.
pub fn install_desktop_integration() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        install_desktop_file()?;
        install_icon()?;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn install_desktop_file() -> Result<()> {
    let home = dirs::home_dir().context("Could not locate home directory")?;
    let applications_dir = home.join(".local/share/applications");

    // Create directory if it doesn't exist
    fs::create_dir_all(&applications_dir)
        .context("Failed to create ~/.local/share/applications")?;

    let desktop_file = applications_dir.join("podfetch.desktop");

    // Desktop file content with proper XDG spec
    let desktop_content = r#"[Desktop Entry]
Version=1.0
Type=Application
Name=PodFetch
Comment=CLI/TUI podcast downloader
Icon=podfetch
Exec=podfetch %U
Terminal=true
Categories=Utility;Audio;
Keywords=podcast;downloader;rss;
MimeType=application/rss+xml;
"#;

    fs::write(&desktop_file, desktop_content)
        .context("Failed to write .desktop file")?;

    // Make desktop file executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o644);
        fs::set_permissions(&desktop_file, perms)
            .context("Failed to set permissions on .desktop file")?;
    }

    log::info!(
        "Installed desktop file to {}",
        desktop_file.display()
    );

    // Try to update desktop database (if available)
    if which::which("update-desktop-database").is_ok() {
        let _ = std::process::Command::new("update-desktop-database")
            .arg(&applications_dir)
            .output();
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn install_icon() -> Result<()> {
    let home = dirs::home_dir().context("Could not locate home directory")?;

    // Standard XDG icon directory: ~/.local/share/icons/hicolor/256x256/apps/podfetch.png
    let icons_dir = home.join(".local/share/icons/hicolor/256x256/apps");
    fs::create_dir_all(&icons_dir).context("Failed to create icon directory")?;

    let target_icon = icons_dir.join("podfetch.png");

    // Try to find and copy icon.png
    // First try relative to current executable
    let exe_path = std::env::current_exe()
        .context("Could not determine executable path")?;
    let exe_dir = exe_path.parent().context("Could not get executable directory")?;

    // Check multiple possible locations
    let possible_icon_paths = vec![
        exe_dir.join("icon.png"),           // Bundled with binary
        exe_dir.parent()
            .map(|p| p.join("icon.png"))    // One level up
            .unwrap_or_default(),
        PathBuf::from("/opt/podfetch/icon.png"),  // Installation path
    ];

    for icon_path in possible_icon_paths {
        if icon_path.exists() {
            fs::copy(&icon_path, &target_icon)
                .context("Failed to copy icon")?;

            log::info!(
                "Installed icon to {}",
                target_icon.display()
            );

            // Try to update icon cache (if available)
            if which::which("gtk-update-icon-cache").is_ok() {
                let cache_dir = home.join(".local/share/icons/hicolor");
                let _ = std::process::Command::new("gtk-update-icon-cache")
                    .arg(&cache_dir)
                    .output();
            }

            return Ok(());
        }
    }

    // Icon not found, but don't fail - it's non-critical
    log::warn!("Could not find icon.png for desktop integration");
    Ok(())
}

/// Return executable name based on how it was launched
/// Used for the .desktop Exec field
#[allow(dead_code)]
fn get_executable_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "podfetch".to_string())
}
