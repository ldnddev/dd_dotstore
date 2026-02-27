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
