## dd_dotstore v1.2.1

Tagged release of the v1.2 line on `master` (PRs #1–#6). Same features as v1.2.0; crate version is now `1.2.1` so `--version` matches the git tag.

## dd_dotstore v1.2.0

Reliability, advertised-UX, fewer apply steps, working search/ignore, module split, and a release train (theme loader, CLI, docs, CI).

### Trust
- One `rebuild_tree` path for init, `r`, reload, import, and ignore-edit. Dests and LINK/COPY modes survive reload; expand state still resets.
- Idle auto-save (300 ms since last mutation) plus immediate flush after bulk apply, undo, import, and ignore edits. Save failures toast once and retry every 5 s.
- `TerminalGuard` restores raw mode / alternate screen / mouse capture on panic and on every exit path.
- Import restores modes. Missing JSON keys stay additive (`modes`, `ignore_patterns`).

### Advertised behavior
- Shift+click range-selects from the previous highlight, inclusive.
- Overwrite warning lists every real directory (scroll, no cap), files after dirs, `Y apply / other cancel`.
- Click inside confirm/overwrite/plan is a no-op; click outside cancels.
- `q` / `Q` quit. Esc never quits (close modal → clear filter → clear selection).
- Footer label is `q: Quit`.

### Fewer steps
- Smart dest defaults (XDG / `$HOME` / labeled `.linked` fallback) shown dim on the source row until apply. Remove never guesses a dest.
- `s` / `p` open the same plan dialog; highlight is enough when nothing is checked. `x` uses the same target rule.
- Assigned-but-missing dests render as `○` (`Planned`). Other metadata errors stay `?`.

### Search and ignore
- `/` focuses an inline Source-title filter and does not clear it. Nested matches auto-expand; unmatched folders hide. Esc while editing restores the previous filter.
- Ignore editor lists, adds, and deletes patterns; they persist in `.dd_dotstore.json`. Import copies ignores only when the key is present.

### Maintainability
- Modules: `domain/`, `theme.rs`, `ui/`, `input/` (including `source_row_zones` hit-test). `state` / `inputs` shims remain for tests.
- Flatten display clones drop folder children after `has_configured_descendant` is set so `◌` / `●` still draw.
- Tests use `TempRoot` Drop so panics do not leak `/tmp`.

### Theme, CLI, docs, CI
- Theme files parse with `serde_yaml` + validated `ThemeFile` (`version: 1`, every color key, `#RRGGBB`).
- Global theme honors `$XDG_CONFIG_HOME/ldnddev/dd_dotstore_theme.yml` (then `$HOME/.config`).
- `dd_dotstore -V` / `--version` prints `dd_dotstore {CARGO_PKG_VERSION}`.
- MSRV rustc 1.85 (`edition = "2024"`). `install.sh` and README match.
- `ARCHITECTURE.md` rewritten to the live modules. Visual standard patched (footer `q`, Esc, inline filter, planned glyph, hit-test, XDG).
- GitHub Actions CI on `master`: `cargo fmt --check`, `clippy -D warnings`, `cargo test`.

## dd_dotstore v1.1.0

### Highlights
- Full mouse support:
  - Click to highlight, far-left zone to toggle multi-select, tree area to expand/collapse folders.
  - Double-click on name to open destination editor.
  - Scroll wheel over source list or Destinations panel (Shift = faster scroll).
  - Drag/click on scrollbars (now present on both main panels).
  - Shift+click for range multi-select on source.
  - Click Destinations panel entries to jump focus (auto-expands ancestor folders as needed).
  - Click outside modals to cancel; click toasts to dismiss.
- UI polish and information density:
  - Proper Unicode tree structure (├─ └─ │  ) instead of simple indents.
  - Counts in titles: Source shows `[X selected / Y]`, Destinations shows total count.
  - Destinations (right) panel is now fully interactive with its own scrollbar and mouse navigation.
  - Folder "●" badges indicating subtrees with configured destinations.
  - Enhanced icons (e.g. ◌ for folders with linked descendants).
- Safety / preview / dry-run:
  - New `p` key opens a detailed **Preview / dry-run** modal listing the exact planned bulk create operations (with dests, modes, and overwrite notes) before any changes.
  - Both Preview and Confirm modals now render the concrete plan (src → dest) for review.
  - Y from preview proceeds to actual apply; any other key safely closes with zero side effects.
- Other:
  - Auto-expand on right-panel navigation.
  - Numerous small robustness and UX fixes from the mouse-control work.

## dd_dotstore v1.0.0

First public release of `dd_dotstore`, a terminal UI for managing dotfile symlinks with safer bulk operations and persistent state.

### Highlights
- Fast TUI workflow for dotfile linking:
  - source tree navigation
  - destination assignment modal
  - bulk create/remove symlinks
- Safety-first symlink operations:
  - overwrite warning modal before replacing real files
  - destination lock protection when an existing symlink is still present
- Persistent config + undo:
  - saves symlink destinations and action history
  - undo stack persisted (last 10 actions)
- Fuzzy search/filter:
  - fuzzy filter in source panel
  - fuzzy destination browser matching while typing
- Import/Export workflows:
  - export snapshots to `~/.dd_dotstore/exports/`
  - import picker sorted newest-first
- Improved usability:
  - `F1` help modal
  - `F2` credits modal
  - status bar key hints
  - destination browser shortcuts (`~`, `g/G`, `PgUp/PgDn`, `Ctrl+U`, `Ctrl+S`)
  - modal rendering fixes (clear background, proper selection highlighting)

### CLI
- Supports project root override:
  - `--root /path/to/dotfiles`
  - positional path (`dd_dotstore /path/to/dotfiles`)
- Built-in help:
  - `dd_dotstore --help`

### Quality
- Release build verified.
- Test suite passing with coverage across modal interactions, import/export, overwrite flow, fuzzy filtering, and undo/persistence paths.
