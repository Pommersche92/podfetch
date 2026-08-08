#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${PROJECT_ROOT}/target/dist"
CARGO_TOML="${PROJECT_ROOT}/Cargo.toml"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_info() { echo -e "${BLUE}ℹ${NC} $*" >&2; }
log_success() { echo -e "${GREEN}✓${NC} $*" >&2; }
log_warning() { echo -e "${YELLOW}⚠${NC} $*" >&2; }
log_step() { echo -e "${CYAN}${BOLD}▶${NC} $*" >&2; }

get_version() { grep '^version = ' "$CARGO_TOML" | head -n1 | sed 's/version = "\(.*\)"/\1/'; }
get_package_name() { grep '^name = ' "$CARGO_TOML" | head -n1 | sed 's/name = "\(.*\)"/\1/'; }

check_gh() {
  if ! command -v gh >/dev/null 2>&1; then
    echo "GitHub CLI is required" >&2
    exit 1
  fi
}

build_release_binary() {
  cd "$PROJECT_ROOT"
  cargo build --release
}

build_windows() {
  if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    log_warning "mingw cross-compiler not found; skipping Windows build"
    return 0
  fi
  if ! rustup target list --installed | grep -q 'x86_64-pc-windows-gnu'; then
    rustup target add x86_64-pc-windows-gnu
  fi

  if [ -f "${PROJECT_ROOT}/icon.png" ] && [ ! -f "${PROJECT_ROOT}/icon.ico" ]; then
    if command -v convert >/dev/null 2>&1; then
      convert icon.png -define icon:auto-resize=256,128,64,48,32,16 icon.ico
    elif command -v magick >/dev/null 2>&1; then
      magick icon.png -define icon:auto-resize=256,128,64,48,32,16 icon.ico
    fi
  fi

  export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
  cargo build --release --target x86_64-pc-windows-gnu
}

create_tarball() {
  local archive_basename="${PACKAGE_NAME}-${VERSION}-x86_64.tar.gz"
  local dir_name="${PACKAGE_NAME}-${VERSION}"
  local staging
  staging=$(mktemp -d)
  mkdir -p "$staging/$dir_name"

  cp "target/release/${PACKAGE_NAME}" "$staging/$dir_name/${PACKAGE_NAME}"
  [ -f LICENSE ] && cp LICENSE "$staging/$dir_name/"
  [ -f README.md ] && cp README.md "$staging/$dir_name/"

  tar -czf "${DIST_DIR}/${archive_basename}" -C "$staging" "$dir_name"
  rm -rf "$staging"
}

create_windows_zip() {
  local win_dir="target/x86_64-pc-windows-gnu/release"
  local archive_basename="${PACKAGE_NAME}-${VERSION}-x86_64-windows.zip"
  local dir_name="${PACKAGE_NAME}-${VERSION}"
  local staging
  staging=$(mktemp -d)
  mkdir -p "$staging/$dir_name"

  if [ -f "${win_dir}/${PACKAGE_NAME}.exe" ]; then
    cp "${win_dir}/${PACKAGE_NAME}.exe" "$staging/$dir_name/${PACKAGE_NAME}.exe"
  else
    log_warning "Windows binary not found; skipping zip"
    rm -rf "$staging"
    return 0
  fi

  [ -f LICENSE ] && cp LICENSE "$staging/$dir_name/"
  [ -f README.md ] && cp README.md "$staging/$dir_name/"

  (cd "$staging" && zip -r "${DIST_DIR}/${archive_basename}" "$dir_name")
  rm -rf "$staging"
}

build_appimage() {
  if [ -x "${PROJECT_ROOT}/scripts/build-appimage.sh" ]; then
    "${PROJECT_ROOT}/scripts/build-appimage.sh" build --variant standard --skip-build
  fi
}

create_github_release() {
  local tag="v${VERSION}"
  local title="🚀 PodFetch v${VERSION}"
  local assets=()

  for f in \
    "${DIST_DIR}/${PACKAGE_NAME}-${VERSION}-x86_64.tar.gz" \
    "${DIST_DIR}/${PACKAGE_NAME}-${VERSION}-x86_64.AppImage" \
    "${DIST_DIR}/${PACKAGE_NAME}-${VERSION}-x86_64-windows.zip"; do
    if [ -f "$f" ]; then
      assets+=("$f")
    fi
  done

  gh release create "$tag" "${assets[@]}" --title "$title" --generate-notes --repo "Pommersche92/podfetch"
}

main() {
  cd "$PROJECT_ROOT"
  check_gh
  VERSION=$(get_version)
  PACKAGE_NAME=$(get_package_name)

  mkdir -p "$DIST_DIR"
  build_release_binary
  build_windows || true
  build_appimage || true
  create_tarball
  create_windows_zip || true
  create_github_release
}

main "$@"
