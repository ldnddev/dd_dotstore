use anyhow::{Context, Result};
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum NodeKind {
    File {
        dest: Option<PathBuf>,
    },
    Folder {
        children: Vec<Node>,
        expanded: bool,
        dest: Option<PathBuf>,
    },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub kind: NodeKind,
    pub selected: bool,
    pub symlink_status: SymlinkStatus,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymlinkStatus {
    None,
    Valid,
    Broken,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Create { src: PathBuf, dest: PathBuf },
    Remove { src: PathBuf, dest: PathBuf },
}

#[derive(Serialize, Deserialize, Default)]
pub struct PersistentData {
    pub symlinks: HashMap<String, String>,
    pub history: Vec<Action>,
}

#[derive(Clone, Debug)]
pub enum Modal {
    EditDest {
        node_idx: usize,
        browser: BrowserState,
    },
    ConfirmBulk {
        action: BulkAction,
    },
    OverwriteWarning {
        conflicts: Vec<Conflict>,
        action_type: BulkAction,
    },
    Error {
        msg: String,
    },
    Search,
    IgnoreEditor {
        selected: usize,
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
}

#[derive(Clone, Copy, Debug)]
pub enum BulkAction {
    Create,
    Remove,
}

#[derive(Clone, Debug)]
pub struct Conflict {
    pub dest: PathBuf,
    pub is_real_file: bool,
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
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                if entry.name == ".." || fuzzy_match(&entry.name, &self.filter) {
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

fn fuzzy_match(candidate: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut q = query.chars().map(|c| c.to_ascii_lowercase());
    let mut want = q.next();
    for ch in candidate.chars().map(|c| c.to_ascii_lowercase()) {
        if let Some(w) = want {
            if ch == w {
                want = q.next();
                if want.is_none() {
                    return true;
                }
            }
        } else {
            return true;
        }
    }
    want.is_none()
}

#[derive(Clone)]
pub struct Theme {
    pub normal: Style,
    pub selected: Style,
    pub highlight: Style,
    pub valid: Style,
    pub broken: Style,
    pub border: Style,
    pub error: Style,
}

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        let base = Color::Rgb(30, 30, 46);
        let mantle = Color::Rgb(24, 24, 37);
        let text = Color::Rgb(205, 214, 244);
        let rosewater = Color::Rgb(245, 224, 220);
        let green = Color::Rgb(166, 227, 161);
        let red = Color::Rgb(243, 139, 168);
        let lavender = Color::Rgb(180, 190, 254);

        Self {
            normal: Style::default().fg(text).bg(base),
            selected: Style::default()
                .fg(lavender)
                .bg(mantle)
                .add_modifier(Modifier::BOLD),
            highlight: Style::default().fg(rosewater).bg(mantle),
            valid: Style::default().fg(green),
            broken: Style::default().fg(red),
            border: Style::default().fg(lavender),
            error: Style::default().fg(red).add_modifier(Modifier::BOLD),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
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
    pub modal: Option<Modal>,
    pub history: VecDeque<Action>,
    pub ignore_patterns: Vec<String>,
    pub theme: Theme,
    pub header_copy: String,
    pub persisted_symlinks: HashMap<String, String>,
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
            ],
            ..Default::default()
        }
    }

    pub fn has_selected(&self) -> bool {
        self.nodes.iter().any(|n| n.selected)
    }

    pub fn update_symlink_statuses(&mut self) -> Result<()> {
        crate::tree::update_symlink_statuses_recursive(&mut self.tree, &self.project_root);
        crate::tree::flatten_visible(self);
        Ok(())
    }
}

pub fn load(path: &Path) -> Result<AppState> {
    let content = std::fs::read_to_string(path).context("Failed to read config")?;
    let data: PersistentData = serde_json::from_str(&content).context("Failed to parse config")?;

    let root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut state = AppState::default_with_root(root);
    state.history = data.history.into_iter().collect();
    state.persisted_symlinks = data.symlinks;
    Ok(state)
}

pub fn save(state: &AppState, path: &Path) -> Result<()> {
    let mut symlinks = HashMap::new();

    fn collect(node: &Node, out: &mut HashMap<String, String>) {
        match &node.kind {
            NodeKind::File { dest: Some(d) } | NodeKind::Folder { dest: Some(d), .. } => {
                out.insert(
                    node.path.to_string_lossy().to_string(),
                    d.to_string_lossy().to_string(),
                );
            }
            NodeKind::File { dest: None } => {}
            NodeKind::Folder { dest: None, .. } => {}
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            for child in children {
                collect(child, out);
            }
        }
    }

    for node in &state.tree {
        collect(node, &mut symlinks);
    }

    let data = PersistentData {
        symlinks,
        history: state.history.iter().cloned().collect(),
    };

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(path, json)?;
    Ok(())
}
