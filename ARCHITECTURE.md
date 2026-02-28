# dd_dotstore Architecture

TUI application for managing Linux dotfiles symlinks using Rust + ratatui.

## Goals
- Select project folder (default = current dir)
- Show directory tree of dotfiles on left panel
- Assign symlink destinations via modal browser
- Multi-select items → create/remove symlinks in bulk
- Persist configuration (.dd_dotstore.json)
- Undo (last 10 actions, persisted)
- Filter/search, ignore patterns, import/export
- Catppuccin theme, vim-style keys, status icons

## Project Structure
src/
├── main.rs
├── app.rs          # minimal App struct + new() + run loop glue
├── state.rs        # App state, Node, Modal, Theme, PersistentData
├── tree.rs         # tree building, flatten, find, expand/collapse
├── actions.rs      # create_symlink, remove_symlink, undo, bulk, import/export
├── ui.rs           # full draw/ui function + modal rendering
├── input.rs        # handle_key + sub-handlers (navigation, modal keys, etc.)
└── utils.rs        # small helpers (status_icon, should_ignore, etc.)

## Layout (chosen)
Classic horizontal split
+---------------------------+---------------------------------+
| Source Panel              | Status Panel                    |
|                           |                                 |
| > nvim     ✓  ~/.config/  | Created symlinks:               |
|   init.lua                | • nvim     → ~/.config/nvim     |
|   lua/                    | • .bashrc  → ~/.bashrc          |
| > sway     ✗              | • zsh      → ~/.zshrc           |
|   config                  |                                 |
| .bashrc    ✓              |                                 |
| [filtered: 42/128 items]  |                                 |
+---------------------------+---------------------------------+
| [centered modal / confirm / error popup when active]        |
+-------------------------------------------------------------+


## Core Components

- **App** struct
  - project_root: PathBuf
  - config_path: .dd_dotstore.json
  - tree: Vec<Node> (hierarchical)
  - nodes: Vec<Node> (flattened visible)
  - list_state, status_list_state
  - filter: String
  - modal: Option<Modal>
  - history: VecDeque<Action> (max 10)
  - ignore_patterns: Vec<String>
  - theme: Theme (Catppuccin)

- **Node**
  - name, path (relative), kind (File/Folder), selected, symlink_status

- **NodeKind**
  - File { dest: Option<PathBuf> }
  - Folder { children: Vec<Node>, expanded: bool }

- **SymlinkStatus**: None | Valid | Broken | Unknown

- **Action**: Create { src, dest } | Remove { src }

- **Modal** variants
  - EditDest { node_idx, browser }
  - ConfirmBulk { action: BulkAction }
  - Error { msg }
  - Search
  - IgnoreEditor { selected }

- **BrowserState** (destination picker)
  - current path, entries, selected

## Key Bindings (main)

- q / Esc          → quit
- j/k ↑↓           → navigate
- space            → toggle select file/folder
- enter            → edit destination (file/folder)
- h/l or ←/→       → collapse/expand folder
- e                → edit destination (file/folder)
- s                → bulk create (confirm)
- x                → bulk remove (confirm)
- u                → undo last action
- /                → open filter
- r                → reload tree
- I                → open ignore patterns editor
- i / E            → import / export config

## Persistence
- .dd_dotstore.json in project root
- symlinks: rel_src → abs_dest
- history: last 10 actions
- saved on exit + after changes

## Features
- Default ignores: .git, node_modules, target, __pycache__, .dd_dotstore.json, .DS_Store
- Fuzzy filter on source panel
- Status icons: ✓ (valid/green), ✗ (broken/red), ? (unknown/yellow)
- Confirm before bulk create/remove
- Overwrite warning (basic)
- Import/export JSON configs (~/.dd_dotstore/exports/)
- Session-persisted undo (max 10)

## Dependencies
- ratatui
- crossterm
- anyhow
- serde + serde_json
- walkdir
- dirs
- chrono (optional for export timestamp)


For contributions, see README.md.
