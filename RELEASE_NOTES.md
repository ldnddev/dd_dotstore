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
