#!/usr/bin/env bash
#
# AUR deployment script for PodFetch.
# Usage: ./scripts/deploy-aur.sh [OPTIONS]
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
AUR_DIR="${PROJECT_ROOT}/aur"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

# Parse arguments
PUSH=false
PACKAGE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --push)
            PUSH=true
            shift
            ;;
        --package)
            PACKAGE="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Deploy PodFetch PKGBUILD to AUR."
            echo ""
            echo "Options:"
            echo "  --push               Commit and push to AUR (default: dry-run)"
            echo "  --package NAME       Process only this package (podfetch)"
            echo "  -h, --help           Show this help"
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

update_pkgbuild_version() {
    local pkgbuild="$1"
    local new_version="$2"
    sed -i "s/^pkgver=.*/pkgver=${new_version}/" "$pkgbuild"
    sed -i "s/^pkgrel=.*/pkgrel=1/" "$pkgbuild"
    log_info "Updated pkgver to $new_version"
}

update_source_checksum() {
    local pkgbuild="$1"
    local version="$2"
    local url="https://github.com/Pommersche92/podfetch/archive/refs/tags/v${version}.tar.gz"

    log_info "Downloading source tarball to compute sha256: $url"
    local archive="${TMP_DIR}/podfetch-${version}.tar.gz"
    if command -v wget &>/dev/null; then
        wget -qO "$archive" "$url"
    else
        curl -sSfL "$url" -o "$archive"
    fi

    local sha256
    sha256=$(sha256sum "$archive" | awk '{print $1}')
    log_info "sha256: $sha256"

    sed -i "s/^sha256sums=.*/sha256sums=('${sha256}')/" "$pkgbuild"
    log_success "Updated source checksum"
}

generate_srcinfo() {
    local dir="$1"
    log_step "Generating .SRCINFO..."
    (cd "$dir" && makepkg --printsrcinfo > .SRCINFO)
    log_success ".SRCINFO generated"
}

push_to_aur() {
    local aur_repo="$1"
    local pkgbuild_src="$2"
    local version="$3"
    local pkgname
    pkgname=$(basename "$(dirname "$pkgbuild_src")")

    if [ ! -d "$aur_repo" ]; then
        log_error "AUR repo not found: $aur_repo"
        log_info "Set it up with:"
        log_info "  mkdir -p $(dirname "$aur_repo")"
        log_info "  cd $(dirname "$aur_repo")"
        log_info "  git clone ssh://aur@aur.archlinux.org/${pkgname}.git aur-repo"
        return 1
    fi

    cp "$pkgbuild_src" "$aur_repo/PKGBUILD"
    [ -f "$(dirname "$pkgbuild_src")/.SRCINFO" ] && cp "$(dirname "$pkgbuild_src")/.SRCINFO" "$aur_repo/.SRCINFO"

    cd "$aur_repo"

    local has_changes=false
    if [ -n "$(git status --porcelain)" ]; then
        has_changes=true
        log_step "Committing changes for $pkgname..."
        git add PKGBUILD .SRCINFO
        git commit -m "Update to version $version"
    fi

    local unpushed_commits=0
    if git rev-parse --abbrev-ref @{upstream} &>/dev/null; then
        unpushed_commits=$(git rev-list @{upstream}..HEAD 2>/dev/null | wc -l)
    else
        if [ -n "$(git log --oneline 2>/dev/null)" ]; then
            unpushed_commits=1
        fi
    fi

    if [ "$has_changes" = false ] && [ "$unpushed_commits" -eq 0 ]; then
        log_info "No changes to push for $pkgname"
        return 0
    fi

    if [ "$PUSH" = true ]; then
        log_step "Pushing $pkgname to AUR..."
        if git push -u origin master 2>&1; then
            log_success "Pushed $pkgname v$version to AUR"
        elif git push -u origin main 2>&1; then
            log_success "Pushed $pkgname v$version to AUR"
        else
            log_error "Failed to push $pkgname"
            return 1
        fi
    else
        log_warning "Dry-run: changes ready but not pushed (pass --push to push)"
        git log --oneline -1
    fi
}

process_package() {
    local pkgname="$1"
    local version="$2"
    local pkg_dir="${AUR_DIR}/${pkgname}"
    local pkgbuild="${pkg_dir}/PKGBUILD"
    local aur_repo="${pkg_dir}/aur-repo"

    echo ""
    log_step "Processing $pkgname..."

    if [ ! -f "$pkgbuild" ]; then
        pkg_dir="$AUR_DIR"
        pkgbuild="${pkg_dir}/PKGBUILD"
        aur_repo="${pkg_dir}/aur-repo"
    fi

    if [ ! -f "$pkgbuild" ]; then
        log_error "PKGBUILD not found: $pkgbuild"
        return 1
    fi

    update_pkgbuild_version "$pkgbuild" "$version"
    update_source_checksum "$pkgbuild" "$version"

    if command -v makepkg &>/dev/null; then
        generate_srcinfo "$pkg_dir"
    else
        log_warning "makepkg not available — skipping .SRCINFO generation"
    fi

    push_to_aur "$aur_repo" "$pkgbuild" "$version"
}

main() {
    cd "$PROJECT_ROOT"

    local version
    version=$(get_version)

    log_info "Deploying PodFetch v$version to AUR"
    [ "$PUSH" = true ] && log_info "Mode: PUSH" || log_warning "Mode: dry-run (use --push to actually push)"
    echo ""

    if [ -n "$PACKAGE" ]; then
        process_package "$PACKAGE" "$version"
    else
        process_package "podfetch" "$version"
    fi

    echo ""
    log_success "AUR deployment complete"
}

main "$@"
