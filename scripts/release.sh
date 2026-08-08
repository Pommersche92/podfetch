#!/usr/bin/env bash
#
# PodFetch full release pipeline.
# Usage: ./scripts/release.sh [OPTIONS]
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
NC='\033[0m'

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_TOML="${PROJECT_ROOT}/Cargo.toml"
SCRIPTS_DIR="${PROJECT_ROOT}/scripts"

# Flags
EXECUTE=false
SKIP_TESTS=false
SKIP_CRATES=false
SKIP_GITHUB=false
SKIP_APPIMAGE=false
SKIP_AUR=false
DRAFT_GITHUB=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --execute)
            EXECUTE=true
            shift
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --skip-crates)
            SKIP_CRATES=true
            shift
            ;;
        --skip-github)
            SKIP_GITHUB=true
            shift
            ;;
        --skip-appimage)
            SKIP_APPIMAGE=true
            shift
            ;;
        --skip-aur)
            SKIP_AUR=true
            shift
            ;;
        --draft-github)
            DRAFT_GITHUB=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Run the full PodFetch release pipeline."
            echo ""
            echo "Options:"
            echo "  --execute            Actually run each step (default: dry-run)"
            echo "  --skip-tests         Skip cargo test"
            echo "  --skip-crates        Skip crates.io publish"
            echo "  --skip-github        Skip GitHub release creation"
            echo "  --skip-appimage      Skip AppImage building"
            echo "  --skip-aur           Skip AUR deployment"
            echo "  --draft-github       Create GitHub release as draft"
            echo "  -h, --help           Show this help"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

log_info()    { echo -e "  ${BLUE}ℹ${NC} $1"; }
log_success() { echo -e "  ${GREEN}✓${NC} $1"; }
log_warning() { echo -e "  ${YELLOW}⚠${NC} $1"; }
log_error()   { echo -e "  ${RED}✗${NC} $1"; }
log_step()    { echo -e "${CYAN}${BOLD}▶ $1${NC}"; }
log_skip()    { echo -e "  ${YELLOW}↷${NC} Skipped: $1"; }

get_version() {
    grep '^version = ' "$CARGO_TOML" | head -n1 | sed 's/version = "\(.*\)"/\1/'
}

confirm() {
    local prompt="$1"
    echo ""
    echo -e "${MAGENTA}${BOLD}?${NC} ${prompt} [y/N] "
    read -r reply
    [[ "$reply" =~ ^[Yy]$ ]]
}

run_step() {
    local step_name="$1"
    shift
    if [ "$EXECUTE" = true ]; then
        "$@"
    else
        log_warning "DRY-RUN: would run: $*"
    fi
}

step_tests() {
    if [ "$SKIP_TESTS" = true ]; then
        log_skip "tests"
        return 0
    fi
    log_step "Running tests..."
    run_step "cargo test" cargo test
    log_success "All tests passed"
}

step_crates_publish() {
    if [ "$SKIP_CRATES" = true ]; then
        log_skip "crates.io publish"
        return 0
    fi

    log_step "Publishing to crates.io..."
    if [ "$EXECUTE" = true ]; then
        if ! confirm "Publish PodFetch v${VERSION} to crates.io?"; then
            log_skip "crates.io publish (user skipped)"
            return 0
        fi
        cargo publish
        log_success "Published to crates.io"
    else
        log_warning "DRY-RUN: would run: cargo publish"
    fi
}

step_github_release() {
    if [ "$SKIP_GITHUB" = true ]; then
        log_skip "GitHub release"
        return 0
    fi

    log_step "Building assets and creating GitHub release..."

    local -a args
    args=()
    [ "$SKIP_APPIMAGE" = true ] && args+=(--skip-appimage)
    [ "$DRAFT_GITHUB" = true ] && args+=(--draft)

    if [ "$EXECUTE" = true ]; then
        if ! confirm "Create GitHub release for PodFetch v${VERSION}?"; then
            log_skip "GitHub release (user skipped)"
            return 0
        fi
        "${SCRIPTS_DIR}/release-github.sh" "${args[@]}"
        log_success "GitHub release created"
    else
        log_warning "DRY-RUN: would run: scripts/release-github.sh ${args[*]:-}"
    fi
}

step_aur_deploy() {
    if [ "$SKIP_AUR" = true ]; then
        log_skip "AUR deployment"
        return 0
    fi

    log_step "Deploying to AUR..."

    if [ "$EXECUTE" = true ]; then
        if ! confirm "Deploy PodFetch to AUR?"; then
            log_skip "AUR deployment (user skipped)"
            return 0
        fi
        "${SCRIPTS_DIR}/deploy-aur.sh" --push
        log_success "AUR deployment complete"
    else
        log_warning "DRY-RUN: would run: scripts/deploy-aur.sh --push"
    fi
}

print_banner() {
    echo ""
    echo -e "${MAGENTA}${BOLD}╔═══════════════════════════════════════╗${NC}"
    echo -e "${MAGENTA}${BOLD}║          PodFetch Release Pipeline     ║${NC}"
    echo -e "${MAGENTA}${BOLD}╚═══════════════════════════════════════╝${NC}"
    echo ""
}

print_plan() {
    echo -e "${BOLD}  Release plan for PodFetch v${VERSION}:${NC}"
    echo ""
    [ "$SKIP_TESTS" = false ]   && echo "    1. cargo test" || echo "    1. [skip] cargo test"
    [ "$SKIP_CRATES" = false ]  && echo "    2. cargo publish (crates.io)" || echo "    2. [skip] crates.io"
    [ "$SKIP_GITHUB" = false ]  && echo "    3. GitHub release (tarball + AppImage)" || echo "    3. [skip] GitHub release"
    [ "$SKIP_AUR" = false ]     && echo "    4. AUR deploy" || echo "    4. [skip] AUR"
    echo ""
    if [ "$EXECUTE" = false ]; then
        echo -e "    ${YELLOW}${BOLD}Mode: DRY-RUN — pass --execute to actually run${NC}"
    else
        echo -e "    ${GREEN}${BOLD}Mode: EXECUTE — this will make real changes${NC}"
    fi
    echo ""
}

print_summary() {
    echo ""
    echo -e "${GREEN}${BOLD}╔═══════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}${BOLD}║           Release Complete! 🚀             ║${NC}"
    echo -e "${GREEN}${BOLD}╚═══════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BOLD}  PodFetch v${VERSION} is now live:${NC}"
    echo ""
    echo "    crates.io: https://crates.io/crates/podfetch"
    echo "    GitHub:    https://github.com/Pommersche92/podfetch/releases"
    echo "    AUR:       https://aur.archlinux.org/packages/podfetch"
    echo ""
    echo -e "${BOLD}  Install commands:${NC}"
    echo "    cargo install podfetch"
    echo "    yay -S podfetch"
    echo ""
}

main() {
    cd "$PROJECT_ROOT"
    VERSION=$(get_version)

    print_banner
    print_plan

    if [ "$EXECUTE" = true ]; then
        if ! confirm "Proceed with release pipeline for PodFetch v${VERSION}?"; then
            echo ""
            log_warning "Release cancelled"
            exit 0
        fi
    fi

    echo ""
    step_tests
    echo ""
    step_crates_publish
    echo ""
    step_github_release
    echo ""
    step_aur_deploy

    print_summary
}

main "$@"
