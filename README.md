# PodFetch

PodFetch is a lightweight command-line podcast downloader written in Rust. It helps you discover podcasts, resolve their RSS feeds, download new episodes, and organize them for local playback or self-hosted media servers such as Jellyfin or Audiobookshelf.

## ✨ Features

- 🔎 Search for podcasts by name using PodcastAddict
- 🔁 Fall back to the Apple iTunes podcast search API when needed
- 📡 Accept either a podcast name or a direct RSS feed URL
- ⬇️ Download new episodes only, tracking already-downloaded episodes in an archive file
- 🏷️ Tag downloaded audio files with title, artist, album, and embedded cover art
- 🧾 Write metadata and cover images for media-library integration
- 📁 Support configurable destination paths with placeholders for podcast name and author
- 🎨 **Interactive TUI (Terminal User Interface)** with ratatui for a polished experience
- 📋 **Interactive wizard** for podcast selection, search confirmation, and download configuration
- 📊 **Real-time download progress UI** with scrollable active downloads and history
- ⌨️ **Keyboard shortcuts** for navigation and control (Tab, ↑↓ arrows, Ctrl-Q)

## 📦 Installation

From the project directory:

```bash
cargo install --path .
```

Or build a release binary manually:

```bash
cargo build --release
./target/release/podfetch --help
```

## 🚀 Release targets

PodFetch now includes local release automation for several distribution formats:

- 🐧 Linux tarball: built as a portable archive for direct use on Linux systems
- 📦 AppImage: created with the AppImage build script for desktop-friendly Linux installs
- 🪟 Windows zip: cross-compiled for Windows using `x86_64-pc-windows-gnu`
- 🐧 Arch Linux AUR: packaging metadata is available in [aur/PKGBUILD](aur/PKGBUILD)

### Build the release artifacts locally

```bash
# Build the Linux binary
cargo build --release

# Build an AppImage
./scripts/build-appimage.sh build

# Build Windows and Linux release assets, then publish a GitHub release
./scripts/release-github.sh
```

### Requirements for Windows and AppImage builds

- Windows cross-compilation requires `x86_64-w64-mingw32-gcc` and the Rust Windows target
- AppImage packaging uses `linuxdeploy`, which is downloaded automatically by the script
- If an icon is present as [icon.png](icon.png), the release scripts will generate [icon.ico](icon.ico) for the Windows executable

### GitHub Pages site

A static landing page for GitHub Pages is available in [docs/index.html](docs/index.html). Publish the repository’s `docs/` folder as the Pages source to make it live.

## 🖥️ User Interface

PodFetch features an interactive Terminal User Interface (TUI) built with [ratatui](https://ratatui.rs/).

### Podcast Setup Wizard

On first run (or when not providing full CLI arguments), you'll be guided through an interactive wizard with 5 steps:

1. **Podcast Input** - Enter a podcast name or RSS feed URL
2. **Search Confirmation** - Choose to search the database or treat input as a URL
3. **Select Result** - Browse and select from search results (if searching)
4. **Output Path** - Confirm the download directory
5. **Review & Start** - Review settings and start the download

**Keyboard shortcuts in the wizard:**
- `Enter` - Move to next step / confirm selection
- `↑↓` - Navigate through search results
- `Y` / `N` / `B` - Quick keys for Yes (search) / No (URL) / Back
- `Ctrl-Q` - Cancel and exit

### Download Progress UI

Once downloads begin, you'll see a real-time progress interface with three sections:

1. **Current Downloads** - Shows active downloads with individual progress bars
2. **Overall Progress Bar** - Visual representation of total download progress
3. **History** - Scrollable list of completed (✅) and failed (❌) downloads

**Keyboard shortcuts during downloads:**
- `Tab` - Switch focus between active downloads and history areas
- `↑↓` - Scroll the focused area (all downloads/history shown with scroll indicators)
- `Ctrl-Q` - Cancel all downloads and exit

## ⚡ Quick Start

On first run, PodFetch will prompt you through an interactive wizard to choose your download settings.

### Example usage

```bash
# Search for a podcast by name and download its episodes
podfetch "Darknet Diaries"

# Download from a known RSS feed directly
podfetch https://example.com/podcast.xml

# Force the input to be treated as a search term
podfetch --search "The Joe Rogan Experience"

# Override the download folder and use a custom template
podfetch -o "~/Music/podcasts/{podcast_name}" "My Podcast"

# Download up to 10 episodes at the same time
podfetch -j 10 "My Podcast"
```

### Command-line options

```bash
podfetch [OPTIONS] [URL_OR_NAME]
```

Options:

- `-s, --search`: force the input to be treated as a search term (skip wizard's search confirmation step)
- `-o, --output <PATH>`: override the download path template (skips output path wizard step)
- `-j, --jobs <N>`: set the maximum number of concurrent downloads (default: 5)

**Note:** When all required arguments are provided (`URL_OR_NAME`, `-s`/URL decision, and optionally `-o` and `-j`), the wizard is skipped entirely and downloads begin immediately.

## ⚙️ How it works

1. **Configuration** - PodFetch loads or creates a configuration file on first launch.
2. **Interactive Setup (if needed)** - If not all required args are provided, launches an interactive wizard:
   - Asks for podcast name or RSS URL
   - Confirms search vs. URL treatment
   - Shows search results for selection (if searching)
   - Confirms output directory
   - Shows final settings review
3. **Input Resolution** - Accepts either:
   - a direct RSS feed URL, or
   - a podcast name, which is searched and resolved to an RSS feed
4. **Output Directory** - Determines the output directory based on the configured template and the podcast metadata.
5. **Parallel Downloads** - Downloads only episodes that have not already been recorded in the archive file, with configurable concurrent download limit.
6. **Metadata Tagging** - Each downloaded file is tagged with metadata and cover art where possible.
7. **Progress Tracking** - Real-time TUI displays active downloads, overall progress, and completed/failed history.

## ⚙️ Configuration

PodFetch stores its configuration in:

- Linux: `~/.local/share/podfetch/config.toml`

Example configuration:

```toml
[config]
download_dir = "~/Downloads/podfetch/{podcast_name}"
```

### Supported path placeholders

- `{podcast_name}`: sanitized podcast title
- `{podcast_author}`: sanitized author name

The tool also expands `~` to your home directory.

## 📂 Output layout

A typical download folder looks like this:

```text
~/Downloads/podfetch/My Podcast/
├── 01 - Episode Title.mp3
├── 02 - Another Episode.m4a
├── cover.jpg
├── folder.jpg
├── metadata.nfo
└── downloaded_episodes.txt
```

## 🛠️ Development

```bash
cargo fmt
cargo build
cargo test
```

## ⚠️ Notes and limitations

- Podcast discovery depends on external services such as PodcastAddict and Apple iTunes search.
- Some podcasts may not expose easily resolvable RSS URLs or may require manual handling.
- Downloads are based on the RSS feed content and the episode metadata from that feed.
- The project is licensed under GPL-2.0.

## 📜 License

This project is licensed under the GPL-2.0 License. See the LICENSE file for details.
