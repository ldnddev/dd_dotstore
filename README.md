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
- A: add dotfile (in List tab)
- S: create symlinks (in Symlink tab)
- Q: quit

## Tests
Run `cargo test` for unit tests on dotfile management.

