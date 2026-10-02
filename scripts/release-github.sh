#!/usr/bin/env bash
#
# GitHub release script for PodFetch.
# Usage: ./scripts/release-github.sh [OPTIONS]
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
DIST_DIR="${PROJECT_ROOT}/target/dist"

# Parse arguments
DRAFT_MODE=false
SKIP_BUILD=false
SKIP_APPIMAGE=false
SKIP_WINDOWS=false
RELEASE_NOTES=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --draft)
            DRAFT_MODE=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --skip-appimage)
            SKIP_APPIMAGE=true
            shift
            ;;
        --skip-windows)
            SKIP_WINDOWS=true
            shift
            ;;
        --notes)
            RELEASE_NOTES="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Build assets and create/update a GitHub release."
            echo ""
            echo "Options:"
            echo "  --draft              Create the release as a draft"
            echo "  --skip-build         Skip the cargo build step"
            echo "  --skip-appimage      Skip AppImage build"
            echo "  --skip-windows       Skip Windows cross-compilation"
            echo "  --notes TEXT         Release notes text"
            echo "  -h, --help           Show this help"
            echo ""
            echo "Requirements:"
            echo "  - gh (GitHub CLI)    https://cli.github.com/"
            echo "  - For AppImage: linuxdeploy (auto-downloaded)"
            echo "  - For Windows: x86_64-w64-mingw32-gcc (MinGW)"
            echo "    Install on Debian/Ubuntu: sudo apt-get install mingw-w64"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

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

check_gh_cli() {
    if ! command -v gh &>/dev/null; then
        log_error "GitHub CLI (gh) is not installed. Install it from https://cli.github.com/"
        exit 1
    fi
}

check_gh_auth() {
    if ! gh auth status &>/dev/null; then
        log_error "Not authenticated to GitHub. Run: gh auth login"
        exit 1
    fi
}

check_release_exists() {
    local tag="$1"
    gh release view "$tag" &>/dev/null
}

build_release() {
    cd "$PROJECT_ROOT"
    export RUSTUP_TOOLCHAIN=stable

    log_step "Building Linux release binary..."
    cargo build --release
    cp "target/release/${PACKAGE_NAME}" "target/release/${PACKAGE_NAME}-built"
    log_success "Linux binary built"
}

create_tarball() {
    local archive_basename="${PACKAGE_NAME}-${VERSION}-x86_64.tar.gz"
    local dir_name="${PACKAGE_NAME}-${VERSION}"
    local tarball="${DIST_DIR}/${archive_basename}"

    log_step "Creating Linux tarball: ${archive_basename}"

    local staging
    staging=$(mktemp -d)
    local staging_dir="${staging}/${dir_name}"
    mkdir -p "$staging_dir"

    cp "target/release/${PACKAGE_NAME}-built" "$staging_dir/${PACKAGE_NAME}"
    [ -f LICENSE ] && cp LICENSE "$staging_dir/"
    [ -f README.md ] && cp README.md "$staging_dir/"

    tar -czf "$tarball" -C "$staging" "$dir_name"
    rm -rf "$staging"

    log_success "Created: ${archive_basename} ($(du -sh "$tarball" | cut -f1))"
}

build_appimage() {
    log_step "Building AppImage..."
    if [ -f "target/release/${PACKAGE_NAME}-built" ]; then
        cp "target/release/${PACKAGE_NAME}-built" "target/release/${PACKAGE_NAME}"
        if "${PROJECT_ROOT}/scripts/build-appimage.sh" build --variant standard --skip-build; then
            log_success "AppImage built"
        else
            log_warning "AppImage build failed — asset will be skipped"
        fi
    else
        log_warning "Release binary not found — skipping AppImage build"
    fi
}

prepare_icon() {
    log_step "Preparing Windows icon..."
    
    local icon_ico="$PROJECT_ROOT/icon.ico"
    local icon_png="$PROJECT_ROOT/icon.png"
    
    # Check if icon.ico already exists
    if [ -f "$icon_ico" ]; then
        log_info "Using existing icon.ico"
        return 0
    fi
    
    # Check if icon.png exists
    if [ ! -f "$icon_png" ]; then
        log_warning "Neither icon.ico nor icon.png found — Windows exe will have no icon"
        return 1
    fi
    
    # Try to convert with ImageMagick
    if command -v convert &>/dev/null; then
        log_info "Converting icon.png to icon.ico..."
        if convert "$icon_png" -define "icon:auto-resize=256,128,64,32,16" "$icon_ico"; then
            log_success "Created icon.ico"
            return 0
        else
            log_warning "ImageMagick conversion failed"
            return 1
        fi
    fi
    
    log_warning "ImageMagick 'convert' not found — cannot create icon.ico"
    log_warning "Install with: sudo apt-get install imagemagick"
    return 1
}

build_windows() {
    log_step "Cross-compiling Windows binary..."
    cd "$PROJECT_ROOT"
    
    # Prepare Windows icon
    prepare_icon
    
    # Check if Windows target is installed
    if ! rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
        log_info "Installing Windows target for Rust..."
        rustup target add x86_64-pc-windows-gnu || return 1
    fi
    
    # Check if mingw is available
    if ! command -v x86_64-w64-mingw32-gcc &>/dev/null; then
        log_warning "MinGW toolchain (x86_64-w64-mingw32-gcc) not found"
        log_warning "Windows build requires: sudo apt-get install mingw-w64"
        return 1
    fi
    
    export RUSTUP_TOOLCHAIN=stable
    
    local build_output
    build_output=$(cargo build --release --target x86_64-pc-windows-gnu 2>&1)
    local build_status=$?
    
    if [ $build_status -ne 0 ]; then
        log_warning "Windows cross-compilation failed — asset will be skipped"
        echo "$build_output" | head -20  # Show first 20 lines of error
        return 1
    fi
    
    log_success "Windows binary built"
}

create_windows_zip() {
    local archive_basename="${PACKAGE_NAME}-${VERSION}-x86_64-windows.zip"
    local zip_file="${DIST_DIR}/${archive_basename}"
    
    log_step "Creating Windows zip: ${archive_basename}"
    
    if ! command -v zip &>/dev/null; then
        log_warning "zip command not found — skipping Windows zip creation"
        return 1
    fi
    
    local staging
    staging=$(mktemp -d)
    local staging_dir="${staging}/${PACKAGE_NAME}-${VERSION}-x86_64"
    mkdir -p "$staging_dir"
    
    cp "target/x86_64-pc-windows-gnu/release/${PACKAGE_NAME}.exe" "$staging_dir/"
    [ -f LICENSE ] && cp LICENSE "$staging_dir/"
    [ -f README.md ] && cp README.md "$staging_dir/"
    
    cd "$staging"
    zip -r "$zip_file" "${PACKAGE_NAME}-${VERSION}-x86_64" > /dev/null
    cd "$PROJECT_ROOT"
    rm -rf "$staging"
    
    log_success "Created: ${archive_basename} ($(du -sh "$zip_file" | cut -f1))"
}

create_github_release() {
    local tag="v$VERSION"
    local title="🚀 PodFetch v${VERSION}"

    log_step "Creating GitHub release: $tag"

    local -a assets
    assets=()

    local f
    for f in \
        "${DIST_DIR}/${PACKAGE_NAME}-${VERSION}-x86_64.tar.gz" \
        "${DIST_DIR}/${PACKAGE_NAME}-${VERSION}-x86_64.AppImage" \
        "${DIST_DIR}/${PACKAGE_NAME}-${VERSION}-x86_64-windows.zip"; do
        if [ -f "$f" ]; then
            assets+=("$f")
            log_info "  + $(basename "$f")"
        fi
    done

    if [ "${#assets[@]}" -eq 0 ]; then
        log_warning "No release assets found in $DIST_DIR"
    fi

    local -a gh_args
    gh_args=(release create "$tag" --title "$title")
    [ "$DRAFT_MODE" = true ] && gh_args+=(--draft)
    [ -n "$RELEASE_NOTES" ] && gh_args+=(--notes "$RELEASE_NOTES") || gh_args+=(--generate-notes)
    gh_args+=(--repo "Pommersche92/podfetch")

    if check_release_exists "$tag"; then
        log_warning "Release $tag already exists — deleting and recreating"
        gh release delete "$tag" --yes --repo "Pommersche92/podfetch" || true
    fi

    gh "${gh_args[@]}" "${assets[@]}"

    log_success "GitHub release created: https://github.com/Pommersche92/podfetch/releases/tag/$tag"
}

main() {
    cd "$PROJECT_ROOT"

    check_gh_cli
    check_gh_auth

    VERSION=$(get_version)
    PACKAGE_NAME=$(get_package_name)

    log_info "Package: $PACKAGE_NAME v$VERSION"
    echo ""

    mkdir -p "$DIST_DIR"

    if [ "$SKIP_BUILD" = false ]; then
        build_release
        echo ""
    fi

    if [ "$SKIP_APPIMAGE" = false ]; then
        build_appimage
        echo ""
    fi

    if [ "$SKIP_WINDOWS" = false ]; then
        if build_windows; then
            create_windows_zip
            echo ""
        else
            log_warning "Windows build skipped"
            echo ""
        fi
    fi

    create_tarball
    echo ""

    create_github_release

    echo ""
    echo -e "${GREEN}${BOLD}✓ Release assets in: target/dist/${NC}"
    ls -lh "$DIST_DIR" 2>/dev/null || true
}

main "$@"
