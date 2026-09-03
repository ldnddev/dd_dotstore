#!/usr/bin/env bash
set -euo pipefail

APP_NAME="dd_dotstore"
THEME_FILE_NAME="dd_dotstore_theme.yml"
THEME_DIR_NAME="ldnddev"
MIN_RUST_VERSION="1.85.0"

usage() {
  cat <<USAGE
Install ${APP_NAME}

Usage:
  ./install.sh [--debug] [--no-build] [--help]
  ./install.sh -uninstall

Environment:
  BINDIR=/path/to/bin     Install directly into this directory.
  PREFIX=/path            Install into PREFIX/bin when BINDIR is not set.
  XDG_CONFIG_HOME=/path    Install theme under XDG_CONFIG_HOME/ldnddev.

Defaults:
  BINDIR=\$HOME/.local/bin
  XDG_CONFIG_HOME=\$HOME/.config

Examples:
  ./install.sh
  ./install.sh -uninstall
  PREFIX=/usr/local ./install.sh
  BINDIR=/opt/bin ./install.sh
USAGE
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

version_ge() {
  local version="$1"
  local required="$2"

  [ "$(printf '%s\n%s\n' "$required" "$version" | sort -V | head -n1)" = "$required" ]
}

BUILD_PROFILE="release"
DO_BUILD=1
UNINSTALL=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    -uninstall|--uninstall)
      UNINSTALL=1
      ;;
    --debug)
      BUILD_PROFILE="debug"
      ;;
    --no-build)
      DO_BUILD=0
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
  shift
done

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [ -n "${BINDIR:-}" ]; then
  INSTALL_DIR="$BINDIR"
elif [ -n "${PREFIX:-}" ]; then
  INSTALL_DIR="${PREFIX%/}/bin"
else
  INSTALL_DIR="$HOME/.local/bin"
fi

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
THEME_DIR="${CONFIG_HOME%/}/${THEME_DIR_NAME}"
THEME_SOURCE="$SCRIPT_DIR/$THEME_FILE_NAME"
THEME_TARGET="$THEME_DIR/$THEME_FILE_NAME"

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

if ! command -v cargo >/dev/null 2>&1; then
  fail "cargo was not found. Install Rust from https://rustup.rs/ and run this script again."
fi

RUST_VERSION="$(rustc --version | awk '{print $2}')"
if ! version_ge "$RUST_VERSION" "$MIN_RUST_VERSION"; then
  fail "Rust ${MIN_RUST_VERSION}+ is required; found ${RUST_VERSION}."
fi

if [ "$DO_BUILD" -eq 1 ]; then
  info "building ${APP_NAME} (${BUILD_PROFILE})"
  if [ "$BUILD_PROFILE" = "release" ]; then
    cargo build --release
  else
    cargo build
  fi
fi

if [ "$BUILD_PROFILE" = "release" ]; then
  SOURCE_BIN="$SCRIPT_DIR/target/release/$APP_NAME"
else
  SOURCE_BIN="$SCRIPT_DIR/target/debug/$APP_NAME"
fi

[ -x "$SOURCE_BIN" ] || fail "built binary not found at $SOURCE_BIN"
[ -f "$THEME_SOURCE" ] || fail "theme file not found at $THEME_SOURCE"

info "installing to ${INSTALL_DIR}/${APP_NAME}"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$SOURCE_BIN" "$INSTALL_DIR/$APP_NAME"

info "installing theme to ${THEME_TARGET}"
mkdir -p "$THEME_DIR"
install -m 0644 "$THEME_SOURCE" "$THEME_TARGET"

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

info "done"
