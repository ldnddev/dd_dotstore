mod action;
mod health;
mod node;
mod persist;

pub use action::*;
pub use health::*;
pub use node::*;
pub use persist::*;

use crate::theme::{Theme, ThemeEditor, ThemeStatus};
use crate::ui::toast::{Toast, ToastLevel};
use ratatui::layout::Rect;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Debug)]
pub enum Modal {
    EditDest {
        node_idx: usize,
        browser: BrowserState,
    },
    Plan {
        action: BulkAction,
        scroll: usize,
    },
    OverwriteWarning {
        conflicts: Vec<Conflict>,
        action_type: BulkAction,
        scroll: usize,
    },
    IgnoreEditor {
        selected: usize,
        draft: String,
    },
    Help,
    Credits,
    ImportPicker {
        files: Vec<PathBuf>,
        selected: usize,
    },
    ExportPicker {
        dir: PathBuf,
        filename: String,
    },
    GroupEditor {
        paths: Vec<PathBuf>,
        draft: String,
    },
    Adopt {
        candidates: Vec<AdoptCandidate>,
        selected: usize,
        checked: Vec<bool>,
    },
    Doctor {
        findings: Vec<DoctorFinding>,
        selected: usize,
        scroll: usize,
    },
    ThemeEditor(ThemeEditor),
}

#[derive(Clone, Debug)]
pub struct BrowserState {
    pub root: PathBuf,
    pub current: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected: usize,
    pub filter: String,
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

impl BrowserState {
    pub fn new() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        let mut state = Self {
            root: home.clone(),
            current: home,
            entries: vec![],
            selected: 0,
            filter: String::new(),
        };
        state.refresh_entries();
        state
    }

    pub fn refresh_entries(&mut self) {
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        if self.current != self.root {
            dirs.push(DirEntry {
                name: "..".to_string(),
                is_dir: true,
            });
        }

        if let Ok(read_dir) = fs::read_dir(&self.current) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let item = DirEntry {
                    name,
                    is_dir: file_type.is_dir(),
                };
                if item.is_dir {
                    dirs.push(item);
                } else {
                    files.push(item);
                }
            }
        }

        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));
        dirs.extend(files);
        self.entries = dirs;
        self.clamp_selected();
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let query: Vec<char> = self
            .filter
            .chars()
            .map(|c| c.to_ascii_lowercase())
            .collect();
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                if entry.name == ".." || crate::tree::fuzzy_match(&entry.name, &query) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn clamp_selected(&mut self) {
        let len = self.filtered_indices().len();
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(len.saturating_sub(1));
        }
    }
}

impl Default for BrowserState {
    fn default() -> Self {
        Self::new()
    }
}

/// Session pointer/layout fields. Persisted domain must not mention Rect.
#[derive(Default)]
pub struct PointerState {
    pub last_frame_area: Rect,
    pub source_area: Rect,
    pub status_area: Rect,
    pub current_modal_area: Option<Rect>,
    pub toast_area: Option<Rect>,
    pub scrollbar_dragging: bool,
    pub last_mouse_click_pos: Option<(u16, u16, Instant)>,
    pub destination_paths: Vec<PathBuf>,
}

#[derive(Default)]
pub struct AppState {
    pub project_root: PathBuf,
    pub config_path: PathBuf,
    pub tree: Vec<Node>,
    pub nodes: Vec<Node>,
    pub list_state: ratatui::widgets::ListState,
    pub status_list_state: ratatui::widgets::ListState,
    pub filter: String,
    pub filter_editing: bool,
    pub filter_snapshot: String,
    pub modal: Option<Modal>,
    pub toast: Option<Toast>,
    pub history: VecDeque<Action>,
    pub ignore_patterns: Vec<String>,
    pub theme: Theme,
    pub header_copy: String,
    pub persisted_symlinks: HashMap<String, String>,
    pub persisted_modes: HashMap<String, ActionMode>,
    pub persisted_groups: HashMap<String, String>,
    pub dirty_since: Option<Instant>,
    pub persist_retry_at: Option<Instant>,
    pub last_save_error: Option<String>,
    pub theme_status: ThemeStatus,
    pub pointer: PointerState,
}

impl AppState {
    pub fn default_with_root(root: PathBuf) -> Self {
        Self {
            project_root: root.clone(),
            config_path: root.join(".dd_dotstore.json"),
            ignore_patterns: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "__pycache__".into(),
                ".dd_dotstore.json".into(),
                ".DS_Store".into(),
                ".linked".into(),
            ],
            ..Default::default()
        }
    }

    pub fn update_symlink_statuses(&mut self) -> anyhow::Result<()> {
        crate::tree::update_symlink_statuses_recursive(&mut self.tree, &self.project_root);
        crate::tree::flatten_visible(self);
        Ok(())
    }

    pub fn show_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast::new(level, message));
    }

    pub fn clear_expired_toast(&mut self) {
        if self.toast.as_ref().is_some_and(Toast::is_expired) {
            self.toast = None;
        }
    }
}
