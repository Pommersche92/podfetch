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

## � Release targets

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

## �🚀 Quick Start

On first run, PodFetch will prompt you to choose a default download directory template.

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

- `-s, --search`: force the input to be treated as a search term
- `-o, --output <PATH>`: override the download path template
- `-j, --jobs <N>`: set the maximum number of concurrent downloads (default: 5)

## ⚙️ How it works

1. PodFetch loads or creates a configuration file on first launch.
2. It accepts either:
   - a direct RSS feed URL, or
   - a podcast name, which is searched and resolved to an RSS feed
3. It determines an output directory based on the configured template and the podcast metadata.
4. It downloads only episodes that have not already been recorded in the archive file.
5. Each downloaded file is tagged with metadata and cover art where possible.

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
