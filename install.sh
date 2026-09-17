#!/usr/bin/env bash
# Install dd_dotstore for this machine.
#
# Curl (public repo):
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_dotstore/master/install.sh | bash
#
# Private repo (needs GitHub access):
#   export GITHUB_TOKEN="$(gh auth token)"
#   curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" \
#     https://raw.githubusercontent.com/ldnddev/dd_dotstore/master/install.sh | bash
#
# From a source checkout:
#   ./install.sh
set -euo pipefail

APP_NAME="dd_dotstore"
THEME_FILE_NAME="dd_dotstore_theme.yml"
THEME_DIR_NAME="ldnddev"
MIN_RUST_VERSION="1.85.0"
REPO="${REPO:-ldnddev/dd_dotstore}"
GITHUB_BASE="${GITHUB_BASE:-https://github.com/${REPO}}"
GITHUB_API="${GITHUB_API:-https://api.github.com/repos/${REPO}}"

usage() {
  cat <<USAGE
Install ${APP_NAME}

Usage:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash
  curl -fsSL -H "Authorization: Bearer \$GITHUB_TOKEN" \\
    https://raw.githubusercontent.com/${REPO}/master/install.sh | bash
  ./install.sh [options]

Options:
  --from-release         Download the prebuilt package for this machine
  --from-source          Build from a source checkout or a git clone
  --tag TAG              Install this release tag (default: latest)
  --tarball FILE         Install from a local release tarball
  --print-target         Print the detected package target and exit
  --print-url            Print the GitHub download URL and exit
  --debug                Source build only: cargo build without --release
  --no-build             Source build only: reuse an already-built binary
  -uninstall, --uninstall
                         Remove the installed binary and theme
  -h, --help             Show this help

Environment:
  BINDIR=/path/to/bin      Install the binary into this directory.
  PREFIX=/path             Install into PREFIX/bin when BINDIR is not set.
  XDG_CONFIG_HOME=/path    Install the theme under XDG_CONFIG_HOME/ldnddev.
  VERSION / --tag          Release tag to download (v1.4.0 or 1.4.0).
  REPO=owner/name          GitHub repo used for downloads and clones.
  GITHUB_TOKEN=...         Optional; raises GitHub API rate limits.
  TARGET=triple            Override the detected package target.

Defaults:
  BINDIR=\$HOME/.local/bin
  XDG_CONFIG_HOME=\$HOME/.config

Examples:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash -s -- --uninstall
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash -s -- --tag v1.4.0
  PREFIX=/usr/local ./install.sh
  BINDIR=/opt/bin ./install.sh --from-release
USAGE
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

warn() {
  printf 'warning: %s\n' "$*" >&2
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

version_ge() {
  local version="$1"
  local required="$2"

  [ "$(printf '%s\n%s\n' "$required" "$version" | sort -V | head -n1)" = "$required" ]
}

is_piped_or_fd() {
  local src="$1"
  case "$src" in
    '' | - | bash | sh | dash | */bash | */sh | */dash | /dev/fd/* | /proc/self/fd/*)
      return 0
      ;;
  esac
  return 1
}

resolve_script_dir() {
  local src="${BASH_SOURCE[0]:-$0}"

  if is_piped_or_fd "$src"; then
    return 1
  fi
  if [ ! -f "$src" ]; then
    return 1
  fi
  cd -- "$(dirname -- "$src")" && pwd
}

in_source_tree() {
  local dir="${1:-}"
  [ -n "$dir" ] \
    && [ -f "$dir/Cargo.toml" ] \
    && [ -f "$dir/src/main.rs" ] \
    && [ -f "$dir/$THEME_FILE_NAME" ]
}

detect_target() {
  local os arch bits
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m | tr '[:upper:]' '[:lower:]')"
  bits="$(getconf LONG_BIT 2>/dev/null || printf '64')"

  case "$os" in
    linux) ;;
    *)
      fail "unsupported OS: $(uname -s). ${APP_NAME} ships Linux packages only."
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch="x86_64" ;;
    aarch64 | arm64) arch="aarch64" ;;
    *)
      fail "unsupported architecture: $(uname -m)"
      ;;
  esac

  if [ "$bits" != "64" ]; then
    fail "unsupported ${bits}-bit OS; a 64-bit Linux host is required"
  fi

  printf '%s-unknown-linux-musl\n' "$arch"
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    latest | '')
      printf 'latest\n'
      ;;
    v*)
      printf '%s\n' "$tag"
      ;;
    *)
      printf 'v%s\n' "$tag"
      ;;
  esac
}

resolve_github_token() {
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    return
  fi
  if [ -n "${GH_TOKEN:-}" ]; then
    GITHUB_TOKEN="$GH_TOKEN"
    return
  fi
  if command -v gh >/dev/null 2>&1; then
    GITHUB_TOKEN="$(gh auth token 2>/dev/null || true)"
  fi
}

github_api() {
  local url="$1"
  need_cmd curl
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    curl --proto '=https' --tlsv1.2 -fsSL \
      -H "Authorization: Bearer ${GITHUB_TOKEN}" \
      -H "Accept: application/vnd.github+json" \
      -H "X-GitHub-Api-Version: 2022-11-28" \
      "$url" 2>/dev/null
  else
    curl --proto '=https' --tlsv1.2 -fsSL \
      -H "Accept: application/vnd.github+json" \
      "$url" 2>/dev/null
  fi
}

http_download() {
  local url="$1"
  local dest="$2"
  local quiet="${3:-0}"
  local accept="${4:-*/*}"
  local curl_log

  need_cmd curl
  if [ "$quiet" -eq 1 ]; then
    curl_log="/dev/null"
  else
    curl_log="/dev/stderr"
  fi

  if [ -n "${GITHUB_TOKEN:-}" ]; then
    if [ "$quiet" -eq 1 ] || [ ! -t 2 ]; then
      curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --retry-delay 1 \
        -H "Authorization: Bearer ${GITHUB_TOKEN}" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -H "Accept: ${accept}" \
        -o "$dest" "$url" 2>"$curl_log"
    else
      curl --proto '=https' --tlsv1.2 -fL --retry 3 --retry-delay 1 \
        -H "Authorization: Bearer ${GITHUB_TOKEN}" \
        -H "X-GitHub-Api-Version: 2022-11-28" \
        -H "Accept: ${accept}" \
        -o "$dest" "$url"
    fi
  else
    if [ "$quiet" -eq 1 ]; then
      curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --retry-delay 1 \
        -H "Accept: ${accept}" -o "$dest" "$url" 2>/dev/null
    elif [ -t 2 ]; then
      curl --proto '=https' --tlsv1.2 -fL --retry 3 --retry-delay 1 \
        -H "Accept: ${accept}" -o "$dest" "$url"
    else
      curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --retry-delay 1 \
        -H "Accept: ${accept}" -o "$dest" "$url"
    fi
  fi
}

github_not_found_hint() {
  cat >&2 <<HINT
error: could not look up GitHub releases for ${REPO} (404).
This repository is private, so anonymous curl gets a 404.
Log in and retry with:

  export GITHUB_TOKEN="\$(gh auth token)"
  curl -fsSL -H "Authorization: Bearer \$GITHUB_TOKEN" \\
    https://raw.githubusercontent.com/${REPO}/master/install.sh | bash

Or make the repository public if you want the unauthenticated one-liner.
HINT
}

latest_release_tag() {
  local json tag
  json="$(github_api "${GITHUB_API}/releases/latest")" || {
    github_not_found_hint
    exit 1
  }
  tag="$(printf '%s\n' "$json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  [ -n "$tag" ] || fail "could not parse the latest release tag for ${REPO}"
  printf '%s\n' "$tag"
}

release_json() {
  local tag="$1"
  if [ "$tag" = "latest" ]; then
    github_api "${GITHUB_API}/releases/latest"
  else
    github_api "${GITHUB_API}/releases/tags/${tag}"
  fi
}

asset_api_url() {
  local json="$1"
  local name="$2"
  local field="${3:-url}"

  if command -v python3 >/dev/null 2>&1; then
    printf '%s' "$json" | python3 -c '
import json, sys
name, field = sys.argv[1], sys.argv[2]
rel = json.load(sys.stdin)
for asset in rel.get("assets", []):
    if asset.get("name") == name:
        value = asset.get(field) or ""
        if value:
            print(value)
            raise SystemExit(0)
raise SystemExit(1)
' "$name" "$field"
    return
  fi

  # Last-resort parse: GitHub puts "url" before "name" on each asset.
  printf '%s' "$json" | tr ',' '\n' | awk -v name="$name" -v field="$field" '
    $0 ~ "\"" field "\"" {
      sub(/.*:[[:space:]]*"/, "")
      sub(/".*/, "")
      url = $0
    }
    $0 ~ "\"name\"" {
      sub(/.*:[[:space:]]*"/, "")
      sub(/".*/, "")
      if ($0 == name && url != "") {
        print url
        found = 1
        exit
      }
    }
    END { if (!found) exit 1 }
  '
}

resolve_tag() {
  local requested="$1"
  requested="$(normalize_tag "$requested")"
  if [ "$requested" = "latest" ]; then
    latest_release_tag
  else
    printf '%s\n' "$requested"
  fi
}

asset_name() {
  local tag="$1"
  local target="$2"
  printf '%s-%s-%s.tar.gz\n' "$APP_NAME" "$tag" "$target"
}

asset_url() {
  local tag="$1"
  local target="$2"
  printf '%s/releases/download/%s/%s\n' "$GITHUB_BASE" "$tag" "$(asset_name "$tag" "$target")"
}

checksum_url() {
  local tag="$1"
  local target="$2"
  printf '%s.sha256\n' "$(asset_url "$tag" "$target")"
}

make_tempdir() {
  if [ -n "${TMPDIR_INSTALL:-}" ]; then
    printf '%s\n' "$TMPDIR_INSTALL"
    return
  fi
  TMPDIR_INSTALL="$(mktemp -d "${TMPDIR:-/tmp}/${APP_NAME}.XXXXXX")"
  printf '%s\n' "$TMPDIR_INSTALL"
}

cleanup() {
  if [ -n "${TMPDIR_INSTALL:-}" ] && [ -d "${TMPDIR_INSTALL}" ]; then
    rm -rf "${TMPDIR_INSTALL}"
  fi
}
trap cleanup EXIT

verify_checksum() {
  local archive="$1"
  local checksum_file="$2"
  local expected actual archive_dir archive_base

  need_cmd sha256sum
  expected="$(awk '{print $1}' "$checksum_file" | head -n1)"
  [ -n "$expected" ] || fail "could not read checksum from ${checksum_file}"
  actual="$(sha256sum "$archive" | awk '{print $1}')"
  if [ "$expected" != "$actual" ]; then
    fail "checksum mismatch for $(basename "$archive")"
  fi

  archive_dir="$(cd -- "$(dirname -- "$archive")" && pwd)"
  archive_base="$(basename "$archive")"
  if grep -q -- "$archive_base" "$checksum_file" 2>/dev/null; then
    (cd "$archive_dir" && sha256sum -c "$(basename "$checksum_file")") >/dev/null
  fi
  info "checksum OK"
}

install_files() {
  local source_bin="$1"
  local source_theme="$2"

  [ -f "$source_bin" ] || fail "binary not found at $source_bin"
  [ -x "$source_bin" ] || chmod 0755 "$source_bin"
  [ -f "$source_theme" ] || fail "theme file not found at $source_theme"

  info "installing to ${INSTALL_DIR}/${APP_NAME}"
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$source_bin" "$INSTALL_DIR/$APP_NAME"

  info "installing theme to ${THEME_TARGET}"
  mkdir -p "$THEME_DIR"
  install -m 0644 "$source_theme" "$THEME_TARGET"

  if command -v "$INSTALL_DIR/$APP_NAME" >/dev/null 2>&1; then
    info "installed $("$INSTALL_DIR/$APP_NAME" --help | head -n1)"
  else
    info "installed ${APP_NAME}"
  fi

  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      cat <<PATH_NOTE

${INSTALL_DIR} is not on your PATH.
Add this line to your shell profile if you want to run ${APP_NAME} by name:

  export PATH="${INSTALL_DIR}:\$PATH"

PATH_NOTE
      ;;
  esac
}

unpack_tarball() {
  local archive="$1"
  local dest="$2"

  need_cmd tar
  mkdir -p "$dest"
  tar -xzf "$archive" -C "$dest"

  local bin theme
  bin="$(find "$dest" -type f -name "$APP_NAME" | head -n1)"
  theme="$(find "$dest" -type f -name "$THEME_FILE_NAME" | head -n1)"
  [ -n "$bin" ] || fail "tarball does not contain ${APP_NAME}"
  [ -n "$theme" ] || fail "tarball does not contain ${THEME_FILE_NAME}"
  chmod 0755 "$bin"
  printf '%s\t%s\n' "$bin" "$theme"
}

install_from_tarball() {
  local archive="$1"
  local checksum_file="${2:-}"
  local unpacked bin theme

  [ -f "$archive" ] || fail "tarball not found at $archive"
  if [ -n "$checksum_file" ]; then
    verify_checksum "$archive" "$checksum_file"
  fi

  unpacked="$(make_tempdir)/unpacked"
  IFS="$(printf '\t')" read -r bin theme < <(unpack_tarball "$archive" "$unpacked")
  install_files "$bin" "$theme"
}

download_release_package() {
  local tag="$1"
  local target="$2"
  local quiet="${3:-0}"
  local tmp archive sums url sums_url json name sums_name accept

  tmp="$(make_tempdir)"
  name="$(asset_name "$tag" "$target")"
  sums_name="${name}.sha256"
  archive="${tmp}/${name}"
  sums="${archive}.sha256"
  accept="*/*"

  RELEASE_ARCHIVE=""
  RELEASE_CHECKSUM=""

  json="$(release_json "$tag" 2>/dev/null || true)"
  if [ -n "$json" ]; then
    if [ -n "${GITHUB_TOKEN:-}" ]; then
      url="$(asset_api_url "$json" "$name" url || true)"
      sums_url="$(asset_api_url "$json" "$sums_name" url || true)"
      accept="application/octet-stream"
    else
      url="$(asset_api_url "$json" "$name" browser_download_url || true)"
      sums_url="$(asset_api_url "$json" "$sums_name" browser_download_url || true)"
    fi
  fi
  if [ -z "$url" ]; then
    url="$(asset_url "$tag" "$target")"
    sums_url="$(checksum_url "$tag" "$target")"
    accept="*/*"
  fi
  if [ -z "$sums_url" ]; then
    sums_url="$(checksum_url "$tag" "$target")"
  fi

  if [ "$quiet" -eq 0 ]; then
    info "downloading ${name}"
  fi
  if ! http_download "$url" "$archive" "$quiet" "$accept"; then
    return 1
  fi
  if ! http_download "$sums_url" "$sums" 1 "$accept"; then
    if [ "$quiet" -eq 0 ]; then
      warn "checksum file not found for ${name}"
    fi
    return 1
  fi
  RELEASE_ARCHIVE="$archive"
  RELEASE_CHECKSUM="$sums"
}

install_from_release() {
  local requested_tag="$1"
  local target="$2"
  local tag

  need_cmd curl
  tag="$(resolve_tag "$requested_tag")"
  info "using release ${tag} (${target})"

  if ! download_release_package "$tag" "$target" 0; then
    fail "no prebuilt package for ${target} in ${tag}.
Supported Linux packages: x86_64-unknown-linux-musl, aarch64-unknown-linux-musl.
Build from source with:
  curl -fsSL ${GITHUB_BASE}/raw/master/install.sh | bash -s -- --from-source"
  fi
  install_from_tarball "$RELEASE_ARCHIVE" "$RELEASE_CHECKSUM"
}

try_install_from_release() {
  local requested_tag="$1"
  local target="$2"
  local tag

  command -v curl >/dev/null 2>&1 || return 1
  if ! tag="$(resolve_tag "$requested_tag" 2>/dev/null)"; then
    return 1
  fi
  info "trying prebuilt package ${tag} (${target})"
  if ! download_release_package "$tag" "$target" 1; then
    return 1
  fi
  install_from_tarball "$RELEASE_ARCHIVE" "$RELEASE_CHECKSUM"
}

ensure_rust() {
  if ! command -v cargo >/dev/null 2>&1; then
    fail "cargo was not found. Install Rust from https://rustup.rs/ and run this script again."
  fi

  local rust_version
  rust_version="$(rustc --version | awk '{print $2}')"
  if ! version_ge "$rust_version" "$MIN_RUST_VERSION"; then
    fail "Rust ${MIN_RUST_VERSION}+ is required; found ${rust_version}."
  fi
}

build_from_source_dir() {
  local source_dir="$1"
  local source_bin theme_source

  (
    cd "$source_dir"
    ensure_rust
    if [ "$DO_BUILD" -eq 1 ]; then
      info "building ${APP_NAME} (${BUILD_PROFILE})"
      if [ "$BUILD_PROFILE" = "release" ]; then
        if [ -f Cargo.lock ]; then
          cargo build --release --locked
        else
          cargo build --release
        fi
      else
        cargo build
      fi
    fi
  )

  if [ "$BUILD_PROFILE" = "release" ]; then
    source_bin="$source_dir/target/release/$APP_NAME"
  else
    source_bin="$source_dir/target/debug/$APP_NAME"
  fi
  theme_source="$source_dir/$THEME_FILE_NAME"
  [ -x "$source_bin" ] || fail "built binary not found at $source_bin"
  install_files "$source_bin" "$theme_source"
}

clone_and_build() {
  local ref="$1"
  local dest source_ref

  need_cmd git
  dest="$(make_tempdir)/src"

  source_ref="$ref"
  if [ "$source_ref" = "latest" ]; then
    if ! source_ref="$(resolve_tag latest 2>/dev/null)"; then
      source_ref=""
    fi
  else
    source_ref="$(normalize_tag "$source_ref")"
    if [ "$source_ref" = "latest" ]; then
      source_ref=""
    fi
  fi

  if [ -n "$source_ref" ]; then
    info "cloning ${GITHUB_BASE} (${source_ref})"
    if ! git clone --depth 1 --branch "$source_ref" "${GITHUB_BASE}.git" "$dest"; then
      warn "tag ${source_ref} not found; cloning default branch"
      git clone --depth 1 "${GITHUB_BASE}.git" "$dest"
    fi
  else
    info "cloning ${GITHUB_BASE}"
    git clone --depth 1 "${GITHUB_BASE}.git" "$dest"
  fi

  build_from_source_dir "$dest"
}

install_from_source() {
  if in_source_tree "${SCRIPT_DIR:-}"; then
    build_from_source_dir "$SCRIPT_DIR"
    return
  fi
  clone_and_build "$TAG"
}

BUILD_PROFILE="release"
DO_BUILD=1
UNINSTALL=0
FROM_RELEASE=0
FROM_SOURCE=0
PRINT_TARGET=0
PRINT_URL=0
TARBALL=""
TAG="${VERSION:-latest}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    -uninstall | --uninstall)
      UNINSTALL=1
      ;;
    --from-release)
      FROM_RELEASE=1
      ;;
    --from-source)
      FROM_SOURCE=1
      ;;
    --tag)
      [ "$#" -ge 2 ] || fail "--tag requires a value"
      TAG="$2"
      shift
      ;;
    --tag=*)
      TAG="${1#*=}"
      ;;
    --tarball)
      [ "$#" -ge 2 ] || fail "--tarball requires a path"
      TARBALL="$2"
      shift
      ;;
    --tarball=*)
      TARBALL="${1#*=}"
      ;;
    --print-target)
      PRINT_TARGET=1
      ;;
    --print-url)
      PRINT_URL=1
      ;;
    --debug)
      BUILD_PROFILE="debug"
      ;;
    --no-build)
      DO_BUILD=0
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
  shift
done

if [ "$FROM_RELEASE" -eq 1 ] && [ "$FROM_SOURCE" -eq 1 ]; then
  fail "use only one of --from-release or --from-source"
fi

resolve_github_token

SCRIPT_DIR=""
if resolved_dir="$(resolve_script_dir)"; then
  SCRIPT_DIR="$resolved_dir"
fi

if [ -n "${BINDIR:-}" ]; then
  INSTALL_DIR="$BINDIR"
elif [ -n "${PREFIX:-}" ]; then
  INSTALL_DIR="${PREFIX%/}/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
fi

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
THEME_DIR="${CONFIG_HOME%/}/${THEME_DIR_NAME}"
THEME_TARGET="$THEME_DIR/$THEME_FILE_NAME"
TARGET="${TARGET:-$(detect_target)}"

if [ "$PRINT_TARGET" -eq 1 ]; then
  printf '%s\n' "$TARGET"
  exit 0
fi

if [ "$PRINT_URL" -eq 1 ]; then
  resolved_tag="$(resolve_tag "$TAG")"
  asset_url "$resolved_tag" "$TARGET"
  exit 0
fi

if [ "$UNINSTALL" -eq 1 ]; then
  info "uninstalling ${APP_NAME}"

  if [ -e "$INSTALL_DIR/$APP_NAME" ] || [ -L "$INSTALL_DIR/$APP_NAME" ]; then
    rm -f "$INSTALL_DIR/$APP_NAME"
    info "removed ${INSTALL_DIR}/${APP_NAME}"
  else
    info "binary not found at ${INSTALL_DIR}/${APP_NAME}"
  fi

  if [ -e "$THEME_TARGET" ] || [ -L "$THEME_TARGET" ]; then
    rm -f "$THEME_TARGET"
    info "removed ${THEME_TARGET}"
  else
    info "theme not found at ${THEME_TARGET}"
  fi

  if [ -d "$THEME_DIR" ]; then
    if rmdir "$THEME_DIR" 2>/dev/null; then
      info "removed empty theme directory ${THEME_DIR}"
    else
      info "left ${THEME_DIR} in place because it contains other files"
    fi
  fi

  info "done"
  exit 0
fi

if [ -n "$TARBALL" ]; then
  checksum_file=""
  if [ -f "${TARBALL}.sha256" ]; then
    checksum_file="${TARBALL}.sha256"
  fi
  install_from_tarball "$TARBALL" "$checksum_file"
  info "done"
  exit 0
fi

if [ "$FROM_RELEASE" -eq 1 ]; then
  install_from_release "$TAG" "$TARGET"
elif [ "$FROM_SOURCE" -eq 1 ]; then
  install_from_source
elif in_source_tree "$SCRIPT_DIR"; then
  install_from_source
else
  if try_install_from_release "$TAG" "$TARGET"; then
    :
  else
    warn "no prebuilt package for ${TARGET}; building from source"
    install_from_source
  fi
fi

info "done"
