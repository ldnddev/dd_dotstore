# dd_dotstore

Simple TUI app to manage dotfiles using symlinks for easy sharing and updates across machines.

## Features
- List, add, remove, edit dotfile paths with custom targets (e.g., ~/.config or ~)
- Create symlinks from repo to custom home locations for flexible placement
- No compression - direct linking for live updates and multi-machine sharing

## Run
```bash
cargo run
```

## Keybindings
- Tab: switch tabs
- Up/Down: select item (in List tab)
- A: add dotfile (prompt name then target)
- E: edit selected (prompt name then target)
- R: remove selected
- S: create symlinks (in Symlink tab, with confirmation)
- F1: toggle keybindings modal
- Q: quit

## Tests
Run `cargo test` for unit tests on dotfile management.

