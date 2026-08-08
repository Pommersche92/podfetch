#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${PROJECT_ROOT}/target/appimage-build"
DIST_DIR="${PROJECT_ROOT}/target/dist"
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
LINUXDEPLOY="${BUILD_DIR}/linuxdeploy-x86_64.AppImage"
VARIANT="both"
SKIP_BUILD=false

log_info() { echo "[info] $*"; }
log_success() { echo "[ok] $*"; }
log_warning() { echo "[warn] $*"; }
log_step() { echo "[step] $*"; }

get_version() {
    grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | head -n1 | sed 's/version = "\(.*\)"/\1/'
}

get_package_name() {
    grep '^name = ' "$PROJECT_ROOT/Cargo.toml" | head -n1 | sed 's/name = "\(.*\)"/\1/'
}

download_linuxdeploy() {
    if [ -f "$LINUXDEPLOY" ] && [ -x "$LINUXDEPLOY" ]; then
        return 0
    fi
    mkdir -p "$BUILD_DIR"
    if command -v wget >/dev/null 2>&1; then
        wget -qO "$LINUXDEPLOY" "$LINUXDEPLOY_URL"
    elif command -v curl >/dev/null 2>&1; then
        curl -sSfL "$LINUXDEPLOY_URL" -o "$LINUXDEPLOY"
    else
        echo "Neither wget nor curl is available" >&2
        exit 1
    fi
    chmod +x "$LINUXDEPLOY"
}

create_appdir() {
    local pkg="$1"
    local variant_name="$2"
    local display_name="$3"
    local appdir="$4"

    rm -rf "$appdir"
    mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/256x256/apps"

    cp "${PROJECT_ROOT}/target/release/$pkg" "$appdir/usr/bin/$pkg"

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
        cp "$icon_src" "$appdir/usr/share/icons/hicolor/256x256/apps/${pkg}.png"
    else
        log_warning "icon.png not found; AppImage will lack an icon"
    fi

    cat > "$appdir/AppRun" <<APPRUN
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "${HERE}/usr/bin/${pkg}" "$@"
APPRUN
    chmod +x "$appdir/AppRun"
}

run_linuxdeploy() {
    local pkg="$1"
    local output_path="$2"
    local appdir="$3"

    mkdir -p "$DIST_DIR"
    export OUTPUT="$output_path"
    if ! ARCH=x86_64 "$LINUXDEPLOY" --appdir "$appdir" --desktop-file "$appdir/usr/share/applications/${pkg}.desktop" --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/${pkg}.png" --output appimage 2>/dev/null; then
        export APPIMAGE_EXTRACT_AND_RUN=1
        ARCH=x86_64 "$LINUXDEPLOY" --appdir "$appdir" --desktop-file "$appdir/usr/share/applications/${pkg}.desktop" --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/${pkg}.png" --output appimage
    fi

    if [ ! -f "$output_path" ]; then
        local found
        found=$(find "$BUILD_DIR" "$PROJECT_ROOT" -maxdepth 1 -name '*.AppImage' -newer "$LINUXDEPLOY" 2>/dev/null | head -n1)
        if [ -n "$found" ]; then
            mv "$found" "$output_path"
        fi
    fi

    chmod +x "$output_path"
}

build_variant() {
    local pkg="$1"
    local version="$2"
    local variant_name="$3"
    local appdir="${BUILD_DIR}/AppDir-${variant_name}"
    local output_path="${DIST_DIR}/${pkg}-${version}-x86_64.AppImage"

    if [ "$SKIP_BUILD" = false ]; then
        cargo build --release
    fi

    create_appdir "$pkg" "$variant_name" "PodFetch" "$appdir"
    run_linuxdeploy "$pkg" "$output_path" "$appdir"
}

main() {
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
                exit 0
                ;;
            *)
                shift
                ;;
        esac
    done

    cd "$PROJECT_ROOT"
    local pkg version
    pkg=$(get_package_name)
    version=$(get_version)

    download_linuxdeploy

    if [ "$VARIANT" = "both" ] || [ "$VARIANT" = "standard" ]; then
        build_variant "$pkg" "$version" "$pkg"
    fi
}

main "$@"
