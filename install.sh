#!/usr/bin/env bash
# Install dd_dotstore for this machine.
#
# Curl:
#   curl -fsSL https://raw.githubusercontent.com/ldnddev/dd_dotstore/master/install.sh | bash
#
# GITHUB_TOKEN is optional and only needed if you hit GitHub API rate limits.
#
# From a source checkout:
#   ./install.sh                 # ask: terminal, window, or both
#   ./install.sh --tui
#   ./install.sh --gui
#   ./install.sh --both
#   ./install.sh --uninstall     # ask the same question
#
# Install writes the chosen binary and its desktop launcher. The theme is
# installed only when it is missing, and only for the terminal or both.
# --uninstall removes the chosen binary and desktop file. It leaves
# dd_dotstore_theme.yml, project maps, and ldnddev config alone. Pass the
# same BINDIR, PREFIX, and XDG_DATA_HOME you used to install.
#
# With no terminal to answer, install selects the terminal app and uninstall
# removes both. INSTALL_GUI=1 is the same as --both.
set -euo pipefail

APP_NAME="dd_dotstore"
GUI_APP_NAME="dd_dotstore-gui"
WM_CLASS="dev.ldnddev.dd_dotstore"
THEME_FILE_NAME="dd_dotstore_theme.yml"
THEME_DIR_NAME="ldnddev"
MIN_RUST_VERSION="1.85.0"
# Compressed archive cap, and per-file cap enforced again after extract.
MAX_ARCHIVE_BYTES=20000000
MAX_MEMBER_BYTES=30000000
REPO="${REPO:-ldnddev/dd_dotstore}"
GITHUB_BASE="${GITHUB_BASE:-https://github.com/${REPO}}"
GITHUB_API="${GITHUB_API:-https://api.github.com/repos/${REPO}}"

usage() {
  cat <<USAGE
Install ${APP_NAME}

Usage:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash
  ./install.sh [options]

Options:
  --tui                  Install the terminal app
  --gui                  Install the desktop window
  --both                 Install the terminal app and the window
  --from-release         Download the prebuilt terminal package for this machine
  --from-source          Build from a source checkout or a git clone
  --tag TAG              Install this release tag (default: latest)
  --tarball FILE         Install from a local release tarball
  --print-target         Print the detected package target and exit
  --print-url            Print the GitHub download URL and exit
  --debug                Source build only: cargo build without --release
  --no-build             Source build only: reuse an already-built binary
  -uninstall, --uninstall
                         Remove the chosen binary and desktop launcher.
                         Leaves dd_dotstore_theme.yml and config in place.
  -h, --help             Show this help

Pass only one of --tui, --gui, or --both. With no terminal to answer, install
selects the terminal app and uninstall removes both. INSTALL_GUI=1 is the
same as --both. The release archive is the terminal app. The window is built
with: cargo build --release -p dd_dotstore_gui

Environment:
  BINDIR=/path/to/bin      Install the binary into this directory.
  PREFIX=/path             Install into PREFIX/bin when BINDIR is not set.
  XDG_CONFIG_HOME=/path    Install the theme under XDG_CONFIG_HOME/ldnddev.
  XDG_DATA_HOME=/path      Install the launcher under XDG_DATA_HOME/applications.
  VERSION / --tag          Release tag to download (v1.6.7 or 1.6.7).
  REPO=owner/name          GitHub repo used for downloads and clones.
  GITHUB_TOKEN=...         Optional; raises GitHub API rate limits.
  TARGET=triple            Override the detected package target.
  INSTALL_GUI=1            Same as --both when no --tui, --gui, or --both.

Defaults:
  BINDIR=\$HOME/.local/bin
  XDG_CONFIG_HOME=\$HOME/.config
  Launcher                 \$HOME/.local/share/applications/${APP_NAME}.desktop
                           when the binary is installed under your home directory.
                           Otherwise the launcher is PREFIX/share/applications.

Examples:
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash -s -- --uninstall
  curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | bash -s -- --tag v1.6.7
  PREFIX=/usr/local ./install.sh
  BINDIR=/opt/bin ./install.sh --from-release
USAGE
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

# ldnddev wordmark, half-block art of the white-on-black mark.
# Printed only on a terminal so a captured log stays plain text.
print_mark() {
  [[ -t 1 ]] || return 0
  printf '\033[40;97m'
  cat <<'EOF'

█▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀█
█  ██                ██                         ██  █
█  ██                ██                         ██  █
█  ██         ▄▄▄▄▄▄ ██     ▄▄▄▄▄▄       ▄▄▄▄▄▄ ██  █
█  ██       ▄██▀▀▀▀▀███  ▄██████████▄  ▄██▀▀▀▀▀███  █
█  ██      ██▀       ██  ██        ██ ██▀       ██  █
█  ██      ██        ██  ██        ██ ██        ██  █
█  ██      ▀█▄       ██  ██        ██ ▀█▄       ██  █
█  ▀█▄▄▄▄▄  ▀██▄▄▄▄██▀   ██        ██  ▀██▄▄▄▄██▀   █
█   ▀▀▀▀▀▀    ▀▀▀▀▀▀     ▀▀        ▀▀    ▀▀▀▀▀▀     █
█                                                   █
█                     ██                            █
█                     ██                            █
█              ▄▄▄▄▄▄ ██     ▄▄▄▄▄▄    ▄▄      ▄▄▄  █
█            ▄██▀▀▀▀▀███   ██▀▀▀▀███   ██     ▄██   █
█           ██▀       ██  ██    ██▀ ▄▄ ██    ▄██    █
█           ██        ██  ██  ▄██   ██ ██    ██     █
█           ▀█▄       ██  ██▄██▀   ▄█▀ ██  ▄██      █
█  ▄▄▄▄▄▄▄▄  ▀██▄▄▄▄██▀    ▀███▄▄▄██▀  ██▄██▀       █
█  ▀▀▀▀▀▀▀▀    ▀▀▀▀▀▀        ▀▀▀▀▀▀    ▀▀▀▀         █
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
EOF
  printf '\033[0m\n'
}

# The device node is mode rw-rw-rw, so -r/-w stay true with no controlling
# terminal. Opening it is the check that matters.
have_tty() {
  { : <>/dev/tty; } 2>/dev/null
}

# Read 1, 2, or 3 from the terminal. stdout is tui, gui, or both.
# $1 is the verb shown in the question. $2 is the default number.
# Returns 1 when the terminal cannot be used. main captures stdout, and a
# failed redirect in that capture does not abort under set -e, so every
# /dev/tty failure has to be an explicit status.
ask_choice() {
  local verb=$1 default=$2 answer choice
  while true; do
    {
      printf '%s which program?\n' "$verb"
      printf '%s\n' "  1) terminal (${APP_NAME})"
      printf '%s\n' "  2) desktop window (${GUI_APP_NAME})"
      printf '%s\n' "  3) both"
      printf 'Choice [%s]: ' "$default"
    } >/dev/tty 2>/dev/null || return 1
    if ! IFS= read -r answer </dev/tty 2>/dev/null; then
      printf '\n' >/dev/tty 2>/dev/null || true
      answer=$default
    fi
    choice=$(printf '%s' "$answer" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]')
    case "$choice" in
      '' ) choice=$default ;;
    esac
    case "$choice" in
      1 | tui | terminal) printf '%s\n' tui; return 0 ;;
      2 | gui | window) printf '%s\n' gui; return 0 ;;
      3 | both) printf '%s\n' both; return 0 ;;
      *) printf '%s\n' "Enter 1, 2, or 3." >/dev/tty 2>/dev/null || return 1 ;;
    esac
  done
}

choose_target() {
  local action=$1 explicit=$2
  if [ -n "$explicit" ]; then
    printf '%s\n' "$explicit"
    return 0
  fi
  if have_tty; then
    if [ "$action" = uninstall ]; then
      ask_choice "Remove" 3 && return 0
    else
      ask_choice "Install" 1 && return 0
    fi
  fi
  if [ "$action" = uninstall ]; then
    printf '==> %s\n' "No terminal for a prompt. Removing both programs." >&2
    printf '%s\n' both
  else
    printf '==> %s\n' "No terminal for a prompt. Installing the terminal app." >&2
    printf '%s\n' tui
  fi
}

# Release tags, asset names, and repo owner/name. Rejects `..` so a tag cannot
# retarget a download URL.
token_ok() {
  case "$1" in
    '' | . | .. | *[!A-Za-z0-9._-]* | *..*)
      return 1
      ;;
  esac
  return 0
}

validate_repo() {
  case "$REPO" in
    */*) ;;
    *) fail "REPO must look like owner/name" ;;
  esac
  case "$REPO" in
    */*/*) fail "REPO must look like owner/name" ;;
  esac
  token_ok "${REPO%%/*}" || fail "REPO must look like owner/name"
  token_ok "${REPO#*/}" || fail "REPO must look like owner/name"
}

# Only the configured repo's GitHub release assets. The JSON parser must not
# be able to send GITHUB_TOKEN or the download to another host.
is_allowed_download_url() {
  local url="$1"
  local gh="https://github.com/${REPO}/releases/download/"
  local api="https://api.github.com/repos/${REPO}/releases/assets/"
  local rest tag name id

  case "$url" in
    *[!A-Za-z0-9:/._-]*)
      return 1
      ;;
  esac
  case "$url" in
    "$gh"*)
      rest="${url#"$gh"}"
      tag="${rest%%/*}"
      name="${rest#*/}"
      [ "$tag" != "$rest" ] || return 1
      case "$name" in
        */*) return 1 ;;
      esac
      token_ok "$tag" || return 1
      token_ok "$name" || return 1
      return 0
      ;;
    "$api"*)
      id="${url#"$api"}"
      case "$id" in
        '' | *[!0-9]*) return 1 ;;
      esac
      return 0
      ;;
  esac
  return 1
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
GitHub releases couldn't be reached. Check the tag name or your network. If you're hitting API rate limits, set GITHUB_TOKEN (for example \`export GITHUB_TOKEN="\$(gh auth token)"\`) and retry the public curl.
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
  token_ok "$tag" || fail "invalid release tag from GitHub"
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
  validate_repo
  token_ok "$tag" || fail "invalid release tag"
  token_ok "$target" || fail "invalid package target"
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

# Menus search XDG_DATA_HOME, then the system data dirs. An install under
# $HOME still publishes the launcher where the session looks.
apps_dir() {
  if [ -n "${XDG_DATA_HOME:-}" ]; then
    printf '%s/applications\n' "${XDG_DATA_HOME%/}"
    return
  fi
  case "$INSTALL_DIR" in
    "$HOME" | "$HOME"/*)
      printf '%s/.local/share/applications\n' "$HOME"
      ;;
    *)
      printf '%s/share/applications\n' "$(dirname "$INSTALL_DIR")"
      ;;
  esac
}

# Quote one Exec argument. A literal % is a field code, so it is written as %%.
desktop_quote() {
  local s="$1"
  case "$s" in
    *$'\n'*) fail "install path contains a newline" ;;
  esac
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//\`/\\\`}"
  s="${s//\$/\\\$}"
  s="${s//%/%%}"
  printf '"%s"' "$s"
}

refresh_desktop_database() {
  local dir="$1"
  if [ -d "$dir" ] && command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$dir" >/dev/null 2>&1 || true
  fi
}

write_desktop_file() {
  local dest="$1"
  local tmpfile
  mkdir -p "$DESKTOP_DIR"
  tmpfile="$(mktemp "$DESKTOP_DIR/.desktop.XXXXXX")"
  if ! cat >"$tmpfile"; then
    rm -f "$tmpfile"
    fail "could not write ${dest}"
  fi
  chmod 0644 "$tmpfile"
  # mv replaces a symlink at the destination instead of writing through it.
  mv -f "$tmpfile" "$dest"
  info "launcher ${dest}"
}

install_tui_launcher() {
  local exec_q
  exec_q="$(desktop_quote "$INSTALL_DIR/$APP_NAME")"
  write_desktop_file "$DESKTOP_TARGET" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=${APP_NAME}
GenericName=Dotfile manager
Comment=Map dotfiles onto home and XDG paths
Exec=${exec_q}
TryExec=${INSTALL_DIR}/${APP_NAME}
Terminal=true
Icon=utilities-terminal
Categories=Utility;
Keywords=dotfiles;symlink;xdg;
StartupNotify=false
EOF
}

install_gui_launcher() {
  local exec_q
  exec_q="$(desktop_quote "$INSTALL_DIR/$GUI_APP_NAME")"
  write_desktop_file "$GUI_DESKTOP_TARGET" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=${APP_NAME} GUI
GenericName=Dotfile manager
Comment=Map dotfiles onto home and XDG paths
Exec=${exec_q}
TryExec=${INSTALL_DIR}/${GUI_APP_NAME}
Terminal=false
Icon=utilities-terminal
Categories=Utility;
Keywords=dotfiles;symlink;xdg;
StartupNotify=true
StartupWMClass=${WM_CLASS}
EOF
}

# Refresh a launcher for every binary still in BINDIR, so installing the
# terminal again does not drop the window launcher.
install_launchers() {
  if [ -e "$INSTALL_DIR/$APP_NAME" ] || [ -L "$INSTALL_DIR/$APP_NAME" ]; then
    install_tui_launcher
  fi
  if [ -e "$INSTALL_DIR/$GUI_APP_NAME" ] || [ -L "$INSTALL_DIR/$GUI_APP_NAME" ]; then
    install_gui_launcher
  fi
  refresh_desktop_database "$DESKTOP_DIR"
}

install_one_binary() {
  local name="$1"
  local source_bin="$2"

  [ -f "$source_bin" ] || fail "binary not found at $source_bin"
  [ -x "$source_bin" ] || chmod 0755 "$source_bin"
  info "installing to ${INSTALL_DIR}/${name}"
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$source_bin" "$INSTALL_DIR/$name"
}

install_theme_if_missing() {
  local source_theme="$1"

  if [ -e "$THEME_TARGET" ] || [ -L "$THEME_TARGET" ]; then
    info "leaving existing theme ${THEME_TARGET}"
    return 0
  fi
  [ -f "$source_theme" ] || fail "theme file not found at $source_theme"
  info "installing theme to ${THEME_TARGET}"
  mkdir -p "$THEME_DIR"
  install -m 0644 "$source_theme" "$THEME_TARGET"
}

note_path() {
  local name="$1"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      cat <<PATH_NOTE

${INSTALL_DIR} is not on your PATH.
The desktop launcher still starts ${name}, because it uses the full path.
Add this line to your shell profile if you want to run ${name} by name:

  export PATH="${INSTALL_DIR}:\$PATH"

PATH_NOTE
      ;;
  esac
}

install_files() {
  local source_bin="$1"
  local source_theme="$2"

  install_one_binary "$APP_NAME" "$source_bin"
  install_theme_if_missing "$source_theme"
  install_launchers

  if command -v "$INSTALL_DIR/$APP_NAME" >/dev/null 2>&1; then
    info "installed $("$INSTALL_DIR/$APP_NAME" --help | head -n1)"
  else
    info "installed ${APP_NAME}"
  fi
  note_path "$APP_NAME"
}

validate_tarball_members() {
  local archive="$1"
  local list line
  local saw_bin=0 saw_theme=0 count=0

  list="$(tar -tzf "$archive")" || fail "could not read tarball"
  while IFS= read -r line || [ -n "$line" ]; do
    [ -n "$line" ] || continue
    count=$((count + 1))
    case "$line" in
      "$APP_NAME" | "./$APP_NAME") saw_bin=1 ;;
      "$THEME_FILE_NAME" | "./$THEME_FILE_NAME") saw_theme=1 ;;
      *) fail "tarball member is not allowed: ${line}" ;;
    esac
  done <<< "$list"
  [ "$count" -eq 2 ] || fail "tarball must contain exactly ${APP_NAME} and ${THEME_FILE_NAME}"
  [ "$saw_bin" -eq 1 ] && [ "$saw_theme" -eq 1 ] || fail "tarball is missing required files"
}

assert_safe_member() {
  local path="$1"
  local label="$2"
  local links size

  if [ -L "$path" ]; then
    fail "refusing symlink in tarball: ${label}"
  fi
  if [ ! -f "$path" ]; then
    fail "tarball member is not a regular file: ${label}"
  fi
  links="$(stat -c %h -- "$path")"
  [ "$links" = "1" ] || fail "refusing hard link in tarball: ${label}"
  size="$(stat -c %s -- "$path")"
  [ "$size" -le "$MAX_MEMBER_BYTES" ] || fail "tarball member is too large: ${label}"
}

extract_tarball() {
  local archive="$1"
  local dest="$2"
  local bytes blocks

  bytes="$(stat -c %s -- "$archive")"
  [ "$bytes" -le "$MAX_ARCHIVE_BYTES" ] || fail "tarball is too large"
  validate_tarball_members "$archive"
  mkdir -p "$dest"
  # bash ulimit -f is 1024-byte blocks on Linux. Stop a gzip bomb while extracting.
  blocks=$((MAX_MEMBER_BYTES / 1024))
  (
    ulimit -f "$blocks"
    tar -xzf "$archive" -C "$dest"
  ) || fail "failed to extract tarball"
  assert_safe_member "$dest/$APP_NAME" "$APP_NAME"
  assert_safe_member "$dest/$THEME_FILE_NAME" "$THEME_FILE_NAME"
}

unpack_tarball() {
  local archive="$1"
  local dest="$2"

  need_cmd tar
  need_cmd stat
  extract_tarball "$archive" "$dest"
  printf '%s\t%s\n' "$dest/$APP_NAME" "$dest/$THEME_FILE_NAME"
}

install_from_tarball() {
  local archive="$1"
  local checksum_file="${2:-}"
  local unpacked bin theme

  [ -f "$archive" ] || fail "tarball not found at $archive"
  if [ "$TARGET_CHOICE" = "gui" ]; then
    fail "the release archive is the terminal app; use --from-source --gui"
  fi
  if [ -n "$checksum_file" ]; then
    verify_checksum "$archive" "$checksum_file"
  fi

  unpacked="$(make_tempdir)/unpacked"
  IFS="$(printf '\t')" read -r bin theme < <(unpack_tarball "$archive" "$unpacked")
  install_files "$bin" "$theme"
  if [ "$TARGET_CHOICE" = "both" ]; then
    warn "the release archive does not include the window"
    if in_source_tree "${SCRIPT_DIR:-}"; then
      info "building the window from this source tree"
      build_gui_only "$SCRIPT_DIR"
    else
      warn "use --from-source --gui to build the window"
    fi
  fi
}

download_release_package() {
  local tag="$1"
  local target="$2"
  local quiet="${3:-0}"
  local tmp archive sums url sums_url json name sums_name accept

  validate_repo
  token_ok "$tag" || fail "invalid release tag"
  token_ok "$target" || fail "invalid package target"

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
  if [ -z "$url" ] || ! is_allowed_download_url "$url"; then
    url="$(asset_url "$tag" "$target")"
    accept="*/*"
  fi
  if [ -z "$sums_url" ] || ! is_allowed_download_url "$sums_url"; then
    sums_url="$(checksum_url "$tag" "$target")"
  fi
  is_allowed_download_url "$url" || fail "refusing unexpected release URL"
  is_allowed_download_url "$sums_url" || fail "refusing unexpected checksum URL"

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

profile_bin_dir() {
  local source_dir="$1"
  local base
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    base="${CARGO_TARGET_DIR%/}"
  else
    base="$source_dir/target"
  fi
  if [ "$BUILD_PROFILE" = "release" ]; then
    printf '%s/release\n' "$base"
  else
    printf '%s/debug\n' "$base"
  fi
}

build_terminal_only() {
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

  source_bin="$(profile_bin_dir "$source_dir")/$APP_NAME"
  theme_source="$source_dir/$THEME_FILE_NAME"
  [ -x "$source_bin" ] || fail "built binary not found at $source_bin"
  install_files "$source_bin" "$theme_source"
}

build_gui_only() {
  local source_dir="$1"
  local source_bin

  (
    cd "$source_dir"
    ensure_rust
    if [ "$DO_BUILD" -eq 1 ]; then
      info "building ${GUI_APP_NAME} (${BUILD_PROFILE})"
      if [ "$BUILD_PROFILE" = "release" ]; then
        cargo build --release -p dd_dotstore_gui
      else
        cargo build -p dd_dotstore_gui
      fi
    fi
  )

  source_bin="$(profile_bin_dir "$source_dir")/$GUI_APP_NAME"
  [ -x "$source_bin" ] || fail "built binary not found at $source_bin"
  install_one_binary "$GUI_APP_NAME" "$source_bin"
  install_launchers
  note_path "$GUI_APP_NAME"
  info "${GUI_APP_NAME} needs Vulkan (libvulkan1 and a driver) and Wayland or X11."
}

build_from_source_dir() {
  local source_dir="$1"
  case "${TARGET_CHOICE:-tui}" in
    gui) build_gui_only "$source_dir" ;;
    both)
      build_terminal_only "$source_dir"
      build_gui_only "$source_dir"
      ;;
    *) build_terminal_only "$source_dir" ;;
  esac
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

  validate_repo
  if [ -n "$source_ref" ]; then
    token_ok "$source_ref" || fail "invalid release tag"
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

remove_path() {
  local path="$1"
  if [ -e "$path" ] || [ -L "$path" ]; then
    rm -f -- "$path"
    info "removed ${path}"
  else
    info "not found at ${path}"
  fi
}

uninstall_choice() {
  info "uninstalling ${TARGET_CHOICE}"
  case "$TARGET_CHOICE" in
    tui)
      remove_path "$INSTALL_DIR/$APP_NAME"
      remove_path "$DESKTOP_TARGET"
      ;;
    gui)
      remove_path "$INSTALL_DIR/$GUI_APP_NAME"
      remove_path "$GUI_DESKTOP_TARGET"
      ;;
    both)
      remove_path "$INSTALL_DIR/$APP_NAME"
      remove_path "$INSTALL_DIR/$GUI_APP_NAME"
      remove_path "$DESKTOP_TARGET"
      remove_path "$GUI_DESKTOP_TARGET"
      ;;
    *) fail "internal: unknown target ${TARGET_CHOICE}" ;;
  esac
  refresh_desktop_database "$DESKTOP_DIR"
  info "left ${THEME_TARGET} in place"
  info "left ldnddev config in place"
  info "done"
}

main() {
  BUILD_PROFILE="release"
  DO_BUILD=1
  UNINSTALL=0
  FROM_RELEASE=0
  FROM_SOURCE=0
  PRINT_TARGET=0
  PRINT_URL=0
  SHOW_HELP=0
  TARBALL=""
  TAG="${VERSION:-latest}"
  TARGET_CHOICE=""

  while [ "$#" -gt 0 ]; do
    case "$1" in
      *=*)
        flag="${1%%=*}"
        rest="${1#*=}"
        flag="$(printf '%s' "$flag" | tr '[:upper:]' '[:lower:]')"
        word="${flag}=${rest}"
        ;;
      *)
        word="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
        ;;
    esac
    case "$word" in
      --tui | --gui | --both)
        if [ -n "$TARGET_CHOICE" ]; then
          fail "pass only one of --tui, --gui, or --both"
        fi
        TARGET_CHOICE="${word#--}"
        ;;
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
        SHOW_HELP=1
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
  if [ -z "$TARGET_CHOICE" ] && [ "${INSTALL_GUI:-}" = "1" ]; then
    TARGET_CHOICE="both"
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
  DESKTOP_DIR="$(apps_dir)"
  DESKTOP_TARGET="$DESKTOP_DIR/${APP_NAME}.desktop"
  GUI_DESKTOP_TARGET="$DESKTOP_DIR/${GUI_APP_NAME}.desktop"
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

  print_mark

  if [ "$SHOW_HELP" -eq 1 ]; then
    usage
    exit 0
  fi

  if [ "$UNINSTALL" -eq 1 ]; then
    TARGET_CHOICE="$(choose_target uninstall "$TARGET_CHOICE")"
    uninstall_choice
    exit 0
  fi

  TARGET_CHOICE="$(choose_target install "$TARGET_CHOICE")"

  if [ -n "$TARBALL" ]; then
    checksum_file=""
    if [ -f "${TARBALL}.sha256" ]; then
      checksum_file="${TARBALL}.sha256"
    fi
    install_from_tarball "$TARBALL" "$checksum_file"
    info "done"
    exit 0
  fi

  if [ "$TARGET_CHOICE" = "gui" ] && [ "$FROM_RELEASE" -eq 1 ]; then
    fail "the release archive is the terminal app; use --from-source --gui"
  fi

  if [ "$FROM_RELEASE" -eq 1 ]; then
    install_from_release "$TAG" "$TARGET"
  elif [ "$FROM_SOURCE" -eq 1 ]; then
    install_from_source
  elif in_source_tree "$SCRIPT_DIR"; then
    install_from_source
  elif [ "$TARGET_CHOICE" = "gui" ]; then
    warn "the release archive is the terminal app; building the window from source"
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
}

# `curl | bash` reads this file from stdin. BASH_SOURCE is unset then, and
# `set -u` aborts on a bare ${BASH_SOURCE[0]}. An empty source means the
# file is being executed, not sourced.
if [[ -z "${BASH_SOURCE[0]:-}" || "${BASH_SOURCE[0]:-}" == "$0" ]]; then
  main "$@"
fi
