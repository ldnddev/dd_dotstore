# dd_dotstore Architecture

TUI application for managing Linux dotfile symlinks and copies using Rust + ratatui.

Normative visual and interaction rules live in **`LDNDDEV_TUI_VISUAL_STANDARD.md`** (theme, header/footer, source panel, layout, mouse hit-test). This file describes the crate as implemented.

## Goals
- Select project folder (default = current dir; `--root` / positional path override)
- Show directory tree of dotfiles on the left; assigned dests on the right
- Assign destinations via modal browser; smart dest shown dim until apply
- Multi-select or highlight → plan dialog → symlink or copy in bulk
- Persist configuration (`.dd_dotstore.json`) with idle auto-save
- Undo (last 10 actions, persisted)
- Inline fuzzy filter, persisted ignore patterns, import/export
- Shared ldnddev TUI theme (`serde_yaml`, XDG lookup), vim-style keys, status icons

## Project Structure

```
src/
├── main.rs              # CLI parser + TerminalGuard + event loop
├── lib.rs               # module exports; `inputs` / `state` / `toast` shims
├── app.rs               # App glue: init, tick, draw, handle_key/mouse, reload
├── domain/
│   ├── mod.rs           # AppState, Modal, BrowserState, PointerState
│   ├── node.rs          # Node, NodeKind, ActionMode, SymlinkStatus
│   ├── action.rs        # Action, BulkAction, Conflict, HISTORY_CAP
│   └── persist.rs       # PersistentData, load, load_persistent, save, persist_now
├── theme.rs             # Theme, ThemeFile (serde_yaml), load_theme
├── tree.rs              # build_tree, flatten_visible, rebuild_tree, assignments
├── actions.rs           # create/remove/undo/import/export, planned_dest, plan lines
├── scan.rs              # HOME/XDG symlink adopt scan + doctor findings
├── input/
│   ├── mod.rs           # handle_key / handle_mouse re-exports
│   ├── keys.rs
│   ├── mouse.rs
│   └── hit_test.rs      # source_row_zones (shared with draw)
└── ui/
    ├── mod.rs           # draw() orchestrates header / panes / footer
    ├── source.rs
    ├── destinations.rs
    ├── modals.rs
    └── toast.rs

tests/
├── common/mod.rs        # TempRoot, key(), mouse helpers
├── dotfiles_test.rs
├── filter.rs
└── persist.rs
```

`state.rs`, `inputs.rs`, `ui.rs`, and `toast.rs` (crate root) are gone. Compatibility shims: `dd_dotstore::inputs`, `dd_dotstore::state`, `dd_dotstore::toast`.

## Layout

Classic horizontal split inside a 3-line header + 1-line footer shell:

```
+---------------------------+---------------------------------+
| Source Panel              | Destinations                    |
| [✓] ○ [LINK] nvim  → dest | • LINK nvim → ~/.config/nvim    |
|     match.txt             |                                 |
+---------------------------+---------------------------------+
| F1:Help  q:Quit  ...                                        |
+-------------------------------------------------------------+
```

Centered modals (plan, overwrite, dest browser, ignore editor, help, credits, import/export) overlay the frame. Toasts sit bottom-right.

## Core Components

- **App** (`src/app.rs`) — `new_with_root` loads JSON + theme, then `rebuild_tree(Persisted)`. `tick` expires toasts and flushes dirty persist.
- **AppState** (`src/domain/mod.rs`) — tree, flattened `nodes`, filter + `filter_editing` / `filter_snapshot`, modal, history, ignores, theme, persist maps, dirty/retry clocks, `pointer: PointerState`.
- **Node** — name, relative path, `NodeKind`, selected, `action_mode`, `symlink_status`, `group`, `has_configured_descendant` (display-only, set at flatten).
- **NodeKind** — `File { dest }` | `Folder { children, expanded, dest }`.
- **SymlinkStatus** — `None` | `Planned` (assigned dest `NotFound`) | `Valid` | `Broken` | `Unknown` (other metadata errors).
- **Action** — `Create` / `Remove` / `Copy` / `RemoveCopy`.
- **Modal** — `EditDest`, `Plan { action, scroll }`, `OverwriteWarning { conflicts, action_type, scroll }`, `IgnoreEditor { selected, draft }`, `Help`, `Credits`, `ImportPicker`, `ExportPicker`, `GroupEditor`, `Adopt`, `Doctor`, `ThemeEditor`.
- **PointerState** — layout rects and mouse session flags. Persisted JSON does not mention `Rect`.
- **Theme** — `serde_yaml` `ThemeFile`; lookup `./dd_dotstore_theme.yml` → `$XDG_CONFIG_HOME/ldnddev/dd_dotstore_theme.yml` → built-in. `XDG_CONFIG_HOME` falls back to `$HOME/.config`. Schema `version: 1`, every color key, `#RRGGBB`.

## Key Bindings (main)

- `q` / `Q`        → quit
- `Esc`            → close modal / restore or clear filter / clear selection (does **not** quit)
- `j`/`k` ↑↓       → navigate
- Space            → toggle checkbox
- Enter / `e`      → edit destination
- `h`/`l` or ←/→   → collapse/expand folder
- `s` / `p`        → same plan dialog for apply (highlight is enough; `Y` applies)
- `x`              → plan remove (never guesses a dest)
- `m` / `M`        → LINK/COPY toggle
- `u`              → undo last action
- `/`              → focus inline source-title filter (does not clear)
- `r`              → reload tree (keeps dests/modes; expand state resets)
- `I`              → ignore editor (list / add / delete, persisted)
- `t`              → set group on highlight or checkboxes
- `T`              → select every item in the highlight's group
- `A`              → reverse-import HOME/XDG symlinks pointing at this project
- `D`              → doctor (broken / planned / unknown / orphan)
- `i` / `E`        → import / export
- `C`              → theme editor (live preview; Tab local/global, default global; Y save; Esc revert)
- `F1` / `F2`      → help / credits

Click inside a confirm/overwrite/plan dialog is a no-op; click outside cancels. Use `Y` to apply.

## Persistence

- `.dd_dotstore.json` in the project root: `symlinks`, `modes`, `history`, `ignore_patterns`, `groups`
- `save` walks the live tree via `collect_assignments` (dest `Some` only; non-default modes only; named groups only)
- Idle auto-save: `mark_dirty` → 300 ms idle in `tick` → `persist_now`. Immediate flush after bulk apply, undo, import, ignore add/delete
- Save failures toast once per message and retry every 5 s. Clean exit: `persist_now_if_dirty` while the TUI is still up, restore the tty, then `eprintln` the save error if any
- `rebuild_tree(LiveTree | Persisted)` is the single rebuild path (init, `r`, reload, import, ignore-edit)
- Missing `modes` / `ignore_patterns` / `groups` keys deserialize to default. Import copies ignores only when `Some`

## Status glyphs

| Status | Glyph |
| --- | --- |
| dest assigned, path NotFound | `○` Planned |
| dest assigned, other metadata Err | `?` Unknown |
| valid link/copy | `✓` |
| broken | `✗` |
| folder has configured descendants | `◌` / `●` (`has_configured_descendant` on the display row) |

Unassigned rows show a dim planned dest (` → path`); Destinations lists assigned dests only.

## CLI

```
dd_dotstore [--root <path>] [path]
dd_dotstore -h | --help
dd_dotstore -V | --version
```

No clap. Parser lives in `src/main.rs`.

## Terminal

`TerminalGuard { raw, alt, mouse }` is constructed first with all flags false. Flags flip on as each step succeeds. `Terminal::new` comes after the guard. `Drop` undoes only what ran (panic unwind included).

## Dependencies

- ratatui, crossterm, anyhow
- serde + serde_json + serde_yaml
- rustc 1.85+ (`edition = "2024"`, `rust-version = "1.85"`)

No `clap`, `walkdir`, `dirs`, or `chrono`.

For contributions, see README.md.
