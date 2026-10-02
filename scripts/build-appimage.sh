#!/usr/bin/env bash
#
# AppImage build script for PodFetch.
# Usage: ./scripts/build-appimage.sh build [--variant standard|both] [--skip-build]
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_TOML="${PROJECT_ROOT}/Cargo.toml"
BUILD_DIR="${PROJECT_ROOT}/target/appimage-build"
DIST_DIR="${PROJECT_ROOT}/target/dist"
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
LINUXDEPLOY="${BUILD_DIR}/linuxdeploy-x86_64.AppImage"
VARIANT="both"
SKIP_BUILD=false

log_info()    { echo -e "${BLUE}ℹ${NC} $1" >&2; }
log_success() { echo -e "${GREEN}✓${NC} $1" >&2; }
log_warning() { echo -e "${YELLOW}⚠${NC} $1" >&2; }
log_error()   { echo -e "${RED}✗${NC} $1" >&2; }
log_step()    { echo -e "${CYAN}${BOLD}▶ $1${NC}" >&2; }

get_version() {
    grep '^version = ' "$CARGO_TOML" | head -n1 | sed 's/version = "\(.*\)"/\1/'
}

get_package_name() {
    grep '^name = ' "$CARGO_TOML" | head -n1 | sed 's/name = "\(.*\)"/\1/'
}

download_linuxdeploy() {
    if [ -f "$LINUXDEPLOY" ] && [ -x "$LINUXDEPLOY" ]; then
        log_info "linuxdeploy already present"
        return 0
    fi

    log_step "Downloading linuxdeploy..."
    mkdir -p "$BUILD_DIR"
    if command -v wget &>/dev/null; then
        wget -qO "$LINUXDEPLOY" "$LINUXDEPLOY_URL"
    elif command -v curl &>/dev/null; then
        curl -sSfL "$LINUXDEPLOY_URL" -o "$LINUXDEPLOY"
    else
        log_error "Neither wget nor curl found"
        exit 1
    fi
    chmod +x "$LINUXDEPLOY"
    log_success "linuxdeploy downloaded"
}

create_appdir() {
    local pkg="$1"
    local variant_name="$2"
    local display_name="$3"
    local appdir="$4"

    log_step "Creating AppDir for ${variant_name}..."
    rm -rf "$appdir"
    mkdir -p "$appdir/usr/bin"
    mkdir -p "$appdir/usr/share/applications"
    mkdir -p "$appdir/usr/share/icons/hicolor/256x256/apps"

    cp "${PROJECT_ROOT}/target/release/${pkg}" "$appdir/usr/bin/${pkg}"

    cat > "$appdir/usr/share/applications/${pkg}.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=${display_name}
Comment=A lightweight CLI podcast downloader for RSS feeds and podcast discovery
Exec=${pkg}
Icon=${pkg}
Categories=Audio;AudioVideo;Network;
Terminal=true
DESKTOP

    local icon_src="${PROJECT_ROOT}/icon.png"
    if [ -f "$icon_src" ]; then
        local icon_dest="$appdir/usr/share/icons/hicolor/256x256/apps/${pkg}.png"
        
        # Check if icon needs resizing (must be exactly 256x256 for linuxdeploy)
        if command -v identify &>/dev/null; then
            local dimensions
            dimensions=$(identify -format "%wx%h" "$icon_src" 2>/dev/null || echo "unknown")
            
            if [ "$dimensions" != "256x256" ]; then
                log_info "Resizing icon from $dimensions to 256x256..."
                if command -v magick &>/dev/null; then
                    magick convert "$icon_src" -resize 256x256 -background none -gravity center -extent 256x256 "$icon_dest" 2>/dev/null
                elif command -v convert &>/dev/null; then
                    convert "$icon_src" -resize 256x256 -background none -gravity center -extent 256x256 "$icon_dest" 2>/dev/null
                else
                    cp "$icon_src" "$icon_dest"
                    log_warning "ImageMagick not found; using icon as-is (may cause linuxdeploy errors)"
                fi
            else
                cp "$icon_src" "$icon_dest"
                log_info "Using icon: icon.png (256x256)"
            fi
        else
            cp "$icon_src" "$icon_dest"
            log_info "Using icon: icon.png"
        fi
    else
        log_warning "icon.png not found; AppImage will lack an icon"
    fi

    cat > "$appdir/AppRun" <<APPRUN
#!/bin/sh
HERE="\$(dirname "\$(readlink -f "\$0")")"
exec "\${HERE}/usr/bin/${pkg}" "\$@"
APPRUN
    chmod +x "$appdir/AppRun"

    log_success "AppDir created"
}

run_linuxdeploy() {
    local pkg="$1"
    local output_path="$2"
    local appdir="$3"

    mkdir -p "$DIST_DIR"
    export OUTPUT="$output_path"
    if ! ARCH=x86_64 "$LINUXDEPLOY" \
            --appdir "$appdir" \
            --desktop-file "$appdir/usr/share/applications/${pkg}.desktop" \
            --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/${pkg}.png" \
            --output appimage 2>&1; then
        log_warning "linuxdeploy failed with FUSE; retrying with --appimage-extract-and-run"
        export APPIMAGE_EXTRACT_AND_RUN=1
        ARCH=x86_64 "$LINUXDEPLOY" \
            --appdir "$appdir" \
            --desktop-file "$appdir/usr/share/applications/${pkg}.desktop" \
            --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/${pkg}.png" \
            --output appimage
    fi

    if [ ! -f "$output_path" ]; then
        local found
        found=$(find "$BUILD_DIR" "$PROJECT_ROOT" -maxdepth 1 -name '*.AppImage' -newer "$LINUXDEPLOY" 2>/dev/null | head -n1)
        if [ -n "$found" ]; then
            mv "$found" "$output_path"
        else
            log_error "Could not locate built AppImage"
            exit 1
        fi
    fi

    chmod +x "$output_path"
    log_success "AppImage: $(basename "$output_path") ($(du -sh "$output_path" | cut -f1))"
}

build_variant() {
    local pkg="$1"
    local version="$2"
    local variant_name="$3"
    local appdir="${BUILD_DIR}/AppDir-${variant_name}"
    local output_path="${DIST_DIR}/${pkg}-${version}-x86_64.AppImage"

    if [ "$SKIP_BUILD" = false ]; then
        log_step "Building ${variant_name} release binary..."
        cargo build --release
    else
        log_info "Skipping cargo build (--skip-build)"
    fi

    create_appdir "$pkg" "$variant_name" "PodFetch" "$appdir"
    log_step "Packaging ${variant_name} AppImage..."
    run_linuxdeploy "$pkg" "$output_path" "$appdir"
}

main() {
    local command="${1:-build}"
    shift || true

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --variant)
                VARIANT="$2"
                shift 2
                ;;
            --skip-build)
                SKIP_BUILD=true
                shift
                ;;
            -h|--help)
                echo "Usage: $0 build [--variant standard|both] [--skip-build]"
                echo ""
                echo "Options:"
                echo "  --variant V    Which variant to build: standard or both (default: both)"
                echo "  --skip-build   Skip cargo build"
                exit 0
                ;;
            *)
                echo -e "${RED}Unknown option: $1${NC}"
                exit 1
                ;;
        esac
    done

    if [ "$command" != "build" ]; then
        echo "Usage: $0 build [--variant standard|both] [--skip-build]"
        exit 1
    fi

    cd "$PROJECT_ROOT"
    local pkg version
    pkg=$(get_package_name)
    version=$(get_version)

    log_info "Building AppImage(s) for $pkg v$version (variant: $VARIANT)"
    echo ""

    download_linuxdeploy

    if [ "$VARIANT" = "both" ] || [ "$VARIANT" = "standard" ]; then
        build_variant "$pkg" "$version" "$pkg"
        echo ""
    fi

    log_success "AppImage build complete"
}

main "$@"
