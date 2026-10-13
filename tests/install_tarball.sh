#!/usr/bin/env bash
# Exercises install.sh archive checks without touching the real home directory.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

make_payload() {
  local dir="$1"
  mkdir -p "$dir"
  printf '#!/bin/sh\necho dd_dotstore-test\n' > "$dir/dd_dotstore"
  chmod 0755 "$dir/dd_dotstore"
  printf 'version: 1\n' > "$dir/dd_dotstore_theme.yml"
}

expect_fail() {
  local needle="$1"
  shift
  if "$@" >"$tmp/out" 2>"$tmp/err"; then
    printf 'expected failure: %s\n' "$*" >&2
    exit 1
  fi
  if ! grep -q -- "$needle" "$tmp/err"; then
    printf 'missing %s in:\n' "$needle" >&2
    cat "$tmp/err" >&2
    exit 1
  fi
}

run_install() {
  XDG_CONFIG_HOME="$1" BINDIR="$2" "$root/install.sh" --tarball "$3"
}

make_payload "$tmp/stage"
tar -czf "$tmp/good.tar.gz" -C "$tmp/stage" dd_dotstore dd_dotstore_theme.yml

run_install "$tmp/cfg" "$tmp/bin" "$tmp/good.tar.gz"
test -x "$tmp/bin/dd_dotstore"
test -f "$tmp/cfg/ldnddev/dd_dotstore_theme.yml"
grep -q 'version: 1' "$tmp/cfg/ldnddev/dd_dotstore_theme.yml"
"$tmp/bin/dd_dotstore" | grep -q 'dd_dotstore-test'

XDG_CONFIG_HOME="$tmp/cfg" BINDIR="$tmp/bin" "$root/install.sh" --uninstall
test ! -e "$tmp/bin/dd_dotstore"
test ! -e "$tmp/cfg/ldnddev/dd_dotstore_theme.yml"

mkdir -p "$tmp/nest/subdir"
make_payload "$tmp/nest/subdir"
tar -czf "$tmp/nest.tar.gz" -C "$tmp/nest" subdir
expect_fail "not allowed" run_install "$tmp/cfg-nest" "$tmp/bin-nest" "$tmp/nest.tar.gz"
test ! -e "$tmp/bin-nest/dd_dotstore"

python3 - "$tmp/slip.tar.gz" "$tmp/stage/dd_dotstore" "$tmp/stage/dd_dotstore_theme.yml" <<'PY'
import io, sys, tarfile
dest, binary, theme = sys.argv[1:]
with tarfile.open(dest, "w:gz") as tar:
    for name in ("../../escaped", "dd_dotstore", "dd_dotstore_theme.yml"):
        src = binary if name.endswith("dd_dotstore") else theme
        data = open(src, "rb").read()
        info = tarfile.TarInfo(name)
        info.size = len(data)
        tar.addfile(info, io.BytesIO(data))
PY
mkdir -p "$tmp/install-tmp"
expect_fail "not allowed" \
  env TMPDIR="$tmp/install-tmp" XDG_CONFIG_HOME="$tmp/cfg-slip" BINDIR="$tmp/bin-slip" \
  "$root/install.sh" --tarball "$tmp/slip.tar.gz"
test ! -e "$tmp/install-tmp/escaped"
test ! -e "$tmp/bin-slip/dd_dotstore"

python3 - "$tmp/link.tar.gz" "$tmp/sentinel" "$tmp/stage/dd_dotstore_theme.yml" <<'PY'
import io, sys, tarfile
dest, sentinel, theme = sys.argv[1:]
with tarfile.open(dest, "w:gz") as tar:
    info = tarfile.TarInfo("dd_dotstore")
    info.type = tarfile.SYMTYPE
    info.linkname = sentinel
    tar.addfile(info)
    data = open(theme, "rb").read()
    theme_info = tarfile.TarInfo("dd_dotstore_theme.yml")
    theme_info.size = len(data)
    tar.addfile(theme_info, io.BytesIO(data))
PY
printf 'ORIGINAL\n' > "$tmp/sentinel"
expect_fail "symlink" run_install "$tmp/cfg-link" "$tmp/bin-link" "$tmp/link.tar.gz"
test "$(cat "$tmp/sentinel")" = "ORIGINAL"
test ! -e "$tmp/bin-link/dd_dotstore"

python3 - "$tmp/hard.tar.gz" "$tmp/stage/dd_dotstore_theme.yml" <<'PY'
import io, sys, tarfile
dest, theme = sys.argv[1:]
with tarfile.open(dest, "w:gz") as tar:
    data = open(theme, "rb").read()
    info = tarfile.TarInfo("dd_dotstore_theme.yml")
    info.size = len(data)
    tar.addfile(info, io.BytesIO(data))
    link = tarfile.TarInfo("dd_dotstore")
    link.type = tarfile.LNKTYPE
    link.linkname = "dd_dotstore_theme.yml"
    tar.addfile(link)
PY
expect_fail "hard link" run_install "$tmp/cfg-hard" "$tmp/bin-hard" "$tmp/hard.tar.gz"

target="$("$root/install.sh" --print-target)"
url="$("$root/install.sh" --print-url --tag v1.2.3)"
test "$url" = "https://github.com/ldnddev/dd_dotstore/releases/download/v1.2.3/dd_dotstore-v1.2.3-${target}.tar.gz"

expect_fail "invalid release tag" "$root/install.sh" --print-url --tag '../evil'
expect_fail "REPO must look like owner/name" env REPO='owner/name/extra' "$root/install.sh" --print-url --tag v1.2.3

printf 'install.sh tarball checks passed\n'
