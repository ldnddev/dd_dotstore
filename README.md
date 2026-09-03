# dd_dotstore

TUI tool to manage Linux dotfiles: select project folder, assign destinations, create symlinks or copy files/folders in bulk, persist config.

## Features

- Auto-detect current dir as project root
- Optional project root override via CLI (`--root <path>` or positional path)
- Tree view of dotfiles (expand/collapse)
- Assign symlink destinations via filesystem browser (`~`, `g/G`, fuzzy filter)
- Multi-select files/folders → bulk link/copy or remove destinations
- Per-item LINK/COPY mode for mixed batches
- Undo last 10 actions (persisted)
- Fuzzy filter on source panel
- Ignore patterns (editable)
- Shared ldnddev theme support
- Theme schema version validation (`version: 1`)
- Startup theme health/status in the footer
- Symlink status icons (✓ valid, ✗ broken)
- Import/export configs (`~/.dd_dotstore/exports/`, newest-first import list)
- Confirm dialogs + overwrite warnings
- Passive info/error notices appear as bottom-right toasts for 5 seconds
- Help (`F1`) and credits (`F2`) modals

## Keybindings
**Main keys**
q / Q           Quit
Esc             Close modal / clear filter / clear selection (does not quit)
F1              Help modal
F2              Credits modal
j / k / ↑ / ↓   Navigate list
Space           Toggle selection (file/folder)
Enter           Edit destination (file/folder)
e               Edit destination (file/folder)
h/l or ←/→      Collapse/expand folder
s               Apply selected LINK/COPY items (confirm)
x               Remove selected destinations (confirm)
p               Preview / dry-run the planned bulk create (shows detailed plan; Y to actually apply)
m               Toggle LINK/COPY for highlighted item
M               Set selected items to the next LINK/COPY mode
u               Undo last action
/               Open filter
r               Reload tree
I               Edit ignore patterns
i / E           Import / Export config


**Modal keys (when interactive dialog open):**
- Arrow / jk    Navigate
- Enter         Confirm / select path
- Esc           Cancel / close
- y             Confirm bulk action
- Delete / d    Remove ignore pattern (in editor)
- Destination browser: type for fuzzy filter, `Ctrl+U` clear, `~` home, `g/G` jump, `PgUp/PgDn` scroll, `Ctrl+S` select current dir

**Mouse controls**
- Click a row: move highlight cursor
- Far-left click on a row (the `[ ]` area): toggle multi-select checkbox (matches Space)
- Click the tree connector area on folders: toggle expand/collapse
- Shift+click a row: range multi-select (sets checkboxes from current highlight to clicked)
- Double-click the name part of a row: open destination editor (activate)
- Scroll wheel while mouse is over the source list: scroll the view (Shift = faster)
- Scroll wheel while mouse is over the right Destinations panel: scroll it
- Click or drag on the scrollbar (right edge of source or Destinations panel): scroll the view
- Click an entry in the right Destinations panel: jump highlight to the matching item in the source tree (auto-expands ancestors if needed)
- Folders in source tree show "●" badge if they contain configured descendants (even if not directly linked)
- Enhanced status icons (◌ for folders with subtree destinations)
- In the destination browser modal: click to move highlight, double-click name to pick/enter a dir, drag or click its scrollbar to scroll the picker list
- Click outside an open modal: close/cancel the modal
- Click inside a confirm/overwrite/preview dialog does nothing — use Y
- Click a toast: dismiss it immediately
- Import picker: click a row to choose it (press Enter to confirm the import)

Additional polish: Source and Destinations titles show selection/total counts; source tree uses proper Unicode connectors (├ └ │) for better structure visibility.

Passive notices do not require input. They appear as toasts in the bottom-right corner and disappear after 5 seconds.

## Run

```bash
# Development
cargo run

# Development with explicit project root
cargo run -- --root /path/to/dotfiles
cargo run -- /path/to/dotfiles

# CLI help
cargo run -- --help

# Release build
cargo build --release
./target/release/dd_dotstore
```

## Theme

`dd_dotstore` uses the shared ldnddev TUI theme system (and overall visual standard) defined in `LDNDDEV_TUI_VISUAL_STANDARD.md` (which incorporates the original `THEME_STRUCTURE_STANDARD.md`).

Theme lookup order:
1. `./dd_dotstore_theme.yml`
2. `~/.config/ldnddev/dd_dotstore_theme.yml`
3. Built-in defaults

Theme files must include the supported schema version:

```yaml
version: 1
colors:
  base_background: "#0F1114"
  # ...
```

At startup, the footer shows theme health/status, including the active source and schema version. If a local or global theme is missing required fields, has an unsupported version, or cannot be parsed, `dd_dotstore` falls back to built-in defaults and shows a warning in the footer.

The credits modal shows the active theme source as `local`, `global`, or `default`.

## Visual Design Standard

`dd_dotstore` follows the shared ldnddev TUI Visual Standard defined in `LDNDDEV_TUI_VISUAL_STANDARD.md`. This single document covers:

- The complete theme system and token mappings
- Header and footer (shell) structure and behavior
- The Source panel (folder navigation) data model, rendering, keyboard, and mouse interactions

All future ldnddev TUI tools (such as dd_ftp) are expected to follow the same standard for a consistent user experience.

See also the older supporting doc: `THEME_STRUCTURE_STANDARD.md` (now incorporated into the master document). `HEADER_FOOTER_GUIDE.md` and `SOURCE_PANEL_GUIDE.md` have been removed.

## Install

```bash
# Build release binary, install to ~/.local/bin, and copy the theme to
# ~/.config/ldnddev/dd_dotstore_theme.yml
./install.sh

# Install somewhere else
PREFIX=/usr/local ./install.sh
BINDIR=/opt/bin ./install.sh

# Use a custom config root for the global theme
XDG_CONFIG_HOME="$HOME/.config" ./install.sh

# Verify
dd_dotstore --help
```

The installer requires Rust 1.70+ and `cargo`. It warns if the target bin directory is not on `PATH`.

## Uninstall

```bash
./install.sh -uninstall
# or
./install.sh --uninstall
```

Uninstall removes the installed binary and `~/.config/ldnddev/dd_dotstore_theme.yml`. The `ldnddev` theme directory is removed only if it is empty.

## Test
```bash
cargo test
# or run specific tests
cargo test tree::build_tree
```

## License
MIT License
(small personal tool, permissive, widely compatible)

## Other Mentions
Config saved as .dd_dotstore.json in project root
Undo history capped at 10, persisted
Ignores common dirs/files by default (.git, node_modules, target, etc.)
Requires Rust 1.70+ and unix-like OS (symlinks)

**Enjoy managing your dotfiles.**
