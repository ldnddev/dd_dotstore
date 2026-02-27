# dd_dotstore

TUI tool to manage Linux dotfiles: select project folder, assign symlink destinations, create/remove symlinks in bulk, persist config.

## Features

- Auto-detect current dir as project root
- Tree view of dotfiles (expand/collapse)
- Assign symlink destinations via filesystem browser (~ by default)
- Multi-select files/folders → bulk create/remove symlinks
- Undo last 10 actions (persisted)
- Fuzzy filter on source panel
- Ignore patterns (editable)
- Catppuccin theme
- Symlink status icons (✓ valid, ✗ broken)
- Import/export configs
- Confirm dialogs + overwrite warnings

## Keybindings
**Main keys**
q / Esc         Quit
j / k / ↑ / ↓   Navigate list
Space           Expand/collapse folder OR toggle file selection
Enter / e       Edit destination (file only)
s               Bulk create symlinks (confirm)
x               Bulk remove symlinks (confirm)
u               Undo last action
/               Open filter
r               Reload tree
I               Edit ignore patterns
i / e           Import / Export config


**Modal keys (when popup open):**
- Arrow / jk    Navigate
- Enter         Confirm / select path
- Esc           Cancel / close
- y             Confirm bulk action
- Delete / d    Remove ignore pattern (in editor)

## Run

```bash
# Development
cargo run

# Release build
cargo build --release
./target/release/dd_dotstore
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