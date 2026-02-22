# dd_dotstore Architecture

## Overview

dd_dotstore is a simple TUI app for managing dotfiles, built with Rust and Ratatui. It allows users to maintain a list of dotfiles, add/remove/edit them, backup to tar.gz, and restore from backups.

## Modules

- **main.rs**: Entry point, terminal setup, event loop, key handling.
- **app.rs**: App state (dotfiles list, current tab, inputs, errors).
- **ui.rs**: Rendering for tabs (List, Backup, Restore), lists, inputs, popups.
- **dotfiles.rs**: Logic for listing, adding, backing up (tar.gz), restoring dotfiles.

## Key Flows

1. **Dotfiles List**: Display, add, remove, edit paths.
2. **Backup**: Select files from list, create tar.gz in ~/backups or specified path.
3. **Restore**: Select tar.gz, extract to home directory (with confirmation).
4. **Event Loop**: Handles keys for navigation, actions, confirmations.

## New Features

- **Dotfiles Management Tab**: List view with add/remove/edit (including custom targets like ~/.config or ~).
- **Symlink Management**: Create soft links from repo to custom home paths for flexible placement (e.g., ~/.config/app or ~/.app).

## Definition of Done

- TUI with tabs for list/backup/restore.
- Logic for dotfile operations.
- Keybindings and rendering.
- Tests for core functions.
- README updates.

For contributions, see README.md.
