# dd_dotstore

TUI tool to manage Linux dotfiles: select project folder, assign symlink destinations, create/remove symlinks in bulk, persist config.

## Features

- Auto-detect current dir as project root
- Optional project root override via CLI (`--root <path>` or positional path)
- Tree view of dotfiles (expand/collapse)
- Assign symlink destinations via filesystem browser (`~`, `g/G`, fuzzy filter)
- Multi-select files/folders → bulk create/remove symlinks
- Undo last 10 actions (persisted)
- Fuzzy filter on source panel
- Ignore patterns (editable)
- Catppuccin theme
- Symlink status icons (✓ valid, ✗ broken)
- Import/export configs (`~/.dd_dotstore/exports/`, newest-first import list)
- Confirm dialogs + overwrite warnings
- Help (`F1`) and credits (`F2`) modals

## Keybindings
**Main keys**
q / Esc         Quit
F1              Help modal
F2              Credits modal
j / k / ↑ / ↓   Navigate list
Space           Expand/collapse folder OR toggle file selection
Enter / e       Edit destination (file only)
s               Bulk create symlinks (confirm)
x               Bulk remove symlinks (confirm)
u               Undo last action
/               Open filter
r               Reload tree
I               Edit ignore patterns
i / E           Import / Export config


**Modal keys (when popup open):**
- Arrow / jk    Navigate
- Enter         Confirm / select path
- Esc           Cancel / close
- y             Confirm bulk action
- Delete / d    Remove ignore pattern (in editor)
- Destination browser: type for fuzzy filter, `Ctrl+U` clear, `~` home, `g/G` jump, `PgUp/PgDn` scroll, `Ctrl+S` select current dir

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

## Install

```bash
# Build release binary
cargo build --release

# Install to ~/.local/bin/dd_dotstore
mkdir -p "$HOME/.local/bin"
cp target/release/dd_dotstore "$HOME/.local/bin/dd_dotstore"
chmod +x "$HOME/.local/bin/dd_dotstore"

# Ensure ~/.local/bin is on PATH (bash)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify
dd_dotstore --help
```

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
