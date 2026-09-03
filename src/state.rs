use crate::toast::{Toast, ToastLevel};
use anyhow::{Context, Result, anyhow};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    #[default]
    Symlink,
    Copy,
}

impl ActionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Symlink => "LINK",
            Self::Copy => "COPY",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Self::Symlink => "linked",
            Self::Copy => "copied",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Symlink => Self::Copy,
            Self::Copy => Self::Symlink,
        }
    }
}

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
    pub action_mode: ActionMode,
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
    Copy { src: PathBuf, dest: PathBuf },
    RemoveCopy { src: PathBuf, dest: PathBuf },
}

#[derive(Serialize, Deserialize, Default)]
pub struct PersistentData {
    pub symlinks: HashMap<String, String>,
    #[serde(default)]
    pub modes: HashMap<String, ActionMode>,
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
    PreviewBulk {
        action: BulkAction,
    },
    OverwriteWarning {
        conflicts: Vec<Conflict>,
        action_type: BulkAction,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeSource {
    Local,
    Global,
    Default,
}

impl ThemeSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
            Self::Default => "default",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeStatusLevel {
    Healthy,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeStatus {
    pub level: ThemeStatusLevel,
    pub message: String,
}

impl ThemeStatus {
    pub fn healthy(source: ThemeSource, version: u64) -> Self {
        Self {
            level: ThemeStatusLevel::Healthy,
            message: format!("Theme OK: {} schema v{}", source.label(), version),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: ThemeStatusLevel::Warning,
            message: message.into(),
        }
    }
}

impl Default for ThemeStatus {
    fn default() -> Self {
        Self::healthy(ThemeSource::Default, SUPPORTED_THEME_VERSION)
    }
}

#[derive(Clone, Copy)]
pub struct ThemeColors {
    pub base_background: Color,
    pub body_background: Color,
    pub modal_background: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_labels: Color,
    pub text_active_focus: Color,
    pub modal_labels: Color,
    pub modal_text: Color,
    pub selected_background: Color,
    pub border_default: Color,
    pub border_active: Color,
    pub scrollbar: Color,
    pub scrollbar_hover: Color,
    pub input_border_default: Color,
    pub input_border_focus: Color,
    pub input_text_default: Color,
    pub input_text_focus: Color,
    pub cursor: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub folders: Color,
    pub files: Color,
    pub links: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            base_background: Color::Rgb(0x0f, 0x11, 0x14),
            body_background: Color::Rgb(0x2a, 0x2d, 0x31),
            modal_background: Color::Rgb(0x1c, 0x1e, 0x21),
            text_primary: Color::Rgb(0xf5, 0xf6, 0xf7),
            text_secondary: Color::Rgb(0x9e, 0xa3, 0xaa),
            text_labels: Color::Rgb(0xff, 0xaf, 0x46),
            text_active_focus: Color::Rgb(0x64, 0xb4, 0xf5),
            modal_labels: Color::Rgb(0x64, 0xb4, 0xf5),
            modal_text: Color::Rgb(0xf5, 0xf6, 0xf7),
            selected_background: Color::Rgb(0x0f, 0x11, 0x14),
            border_default: Color::Rgb(0xf5, 0xf6, 0xf7),
            border_active: Color::Rgb(0x64, 0xb4, 0xf5),
            scrollbar: Color::Rgb(0xff, 0xa0, 0x87),
            scrollbar_hover: Color::Rgb(0x64, 0xb4, 0xf5),
            input_border_default: Color::Rgb(0xf5, 0xf6, 0xf7),
            input_border_focus: Color::Rgb(0x64, 0xb4, 0xf5),
            input_text_default: Color::Rgb(0xf5, 0xf6, 0xf7),
            input_text_focus: Color::Rgb(0x64, 0xb4, 0xf5),
            cursor: Color::Rgb(0x64, 0xb4, 0xf5),
            success: Color::Rgb(0x82, 0xe0, 0xaa),
            warning: Color::Rgb(0xf5, 0xc4, 0x69),
            error: Color::Rgb(0xe5, 0x73, 0x73),
            info: Color::Rgb(0x5d, 0xad, 0xe2),
            folders: Color::Rgb(0x64, 0xb4, 0xf5),
            files: Color::Rgb(0xff, 0xaf, 0x46),
            links: Color::Rgb(0xff, 0xa0, 0x87),
        }
    }
}

#[derive(Clone)]
pub struct Theme {
    pub source: ThemeSource,
    pub version: u64,
    pub colors: ThemeColors,
    pub header_quotes: Vec<String>,
    pub app_shell: Style,
    pub body: Style,
    pub modal: Style,
    pub normal: Style,
    pub secondary: Style,
    pub label: Style,
    pub active_label: Style,
    pub modal_label: Style,
    pub modal_text: Style,
    pub selected: Style,
    pub highlight: Style,
    pub valid: Style,
    pub warning: Style,
    pub broken: Style,
    pub info: Style,
    pub folder: Style,
    pub file: Style,
    pub link: Style,
    pub border: Style,
    pub active_border: Style,
    pub input_border: Style,
    pub input_border_focus: Style,
    pub input_text: Style,
    pub input_text_focus: Style,
    pub cursor: Style,
    pub scrollbar: Style,
    pub scrollbar_hover: Style,
    pub error: Style,
}

impl Theme {
    pub fn from_colors(colors: ThemeColors, source: ThemeSource, version: u64) -> Self {
        Self {
            source,
            version,
            colors,
            header_quotes: vec![],
            app_shell: Style::default()
                .fg(colors.text_primary)
                .bg(colors.base_background),
            body: Style::default()
                .fg(colors.text_primary)
                .bg(colors.body_background),
            modal: Style::default()
                .fg(colors.modal_text)
                .bg(colors.modal_background),
            normal: Style::default()
                .fg(colors.text_primary)
                .bg(colors.body_background),
            secondary: Style::default()
                .fg(colors.text_secondary)
                .bg(colors.body_background),
            label: Style::default()
                .fg(colors.text_labels)
                .bg(colors.body_background),
            active_label: Style::default()
                .fg(colors.text_active_focus)
                .bg(colors.body_background)
                .add_modifier(Modifier::BOLD),
            modal_label: Style::default()
                .fg(colors.modal_labels)
                .bg(colors.modal_background)
                .add_modifier(Modifier::BOLD),
            modal_text: Style::default()
                .fg(colors.modal_text)
                .bg(colors.modal_background),
            selected: Style::default()
                .fg(colors.text_active_focus)
                .bg(colors.selected_background)
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .fg(colors.warning)
                .bg(colors.body_background),
            valid: Style::default()
                .fg(colors.success)
                .bg(colors.body_background),
            warning: Style::default()
                .fg(colors.warning)
                .bg(colors.body_background),
            broken: Style::default().fg(colors.error).bg(colors.body_background),
            info: Style::default().fg(colors.info).bg(colors.body_background),
            folder: Style::default()
                .fg(colors.folders)
                .bg(colors.body_background),
            file: Style::default().fg(colors.files).bg(colors.body_background),
            link: Style::default().fg(colors.links).bg(colors.body_background),
            border: Style::default().fg(colors.border_default),
            active_border: Style::default().fg(colors.border_active),
            input_border: Style::default().fg(colors.input_border_default),
            input_border_focus: Style::default().fg(colors.input_border_focus),
            input_text: Style::default()
                .fg(colors.input_text_default)
                .bg(colors.modal_background),
            input_text_focus: Style::default()
                .fg(colors.input_text_focus)
                .bg(colors.modal_background),
            cursor: Style::default()
                .fg(colors.cursor)
                .bg(colors.modal_background)
                .add_modifier(Modifier::REVERSED),
            scrollbar: Style::default().fg(colors.scrollbar),
            scrollbar_hover: Style::default().fg(colors.scrollbar_hover),
            error: Style::default()
                .fg(colors.error)
                .bg(colors.modal_background)
                .add_modifier(Modifier::BOLD),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        let mut theme = Self::from_colors(
            ThemeColors::default(),
            ThemeSource::Default,
            SUPPORTED_THEME_VERSION,
        );
        theme.header_quotes = DEFAULT_HEADER_QUOTES
            .iter()
            .map(|s| s.to_string())
            .collect();
        theme
    }
}

const THEME_FILE_NAME: &str = "dd_dotstore_theme.yml";
const SUPPORTED_THEME_VERSION: u64 = 1;

const DEFAULT_HEADER_QUOTES: [&str; 5] = [
    "Don't Fear the . (Dot) - Tame It.",
    ". (Dot) file Domination done right.",
    ". (Dot) file management fatigue is real. Or used to be.",
    ". (Dot) file sync setup in seconds - okay, fast.",
    ". (Dot) file management for the Ricer at heart.",
];

pub fn load_theme(project_root: &Path) -> Result<Theme> {
    let local = project_root.join(THEME_FILE_NAME);
    if local.exists() {
        return load_theme_file(&local, ThemeSource::Local);
    }

    if let Some(home) = std::env::var_os("HOME") {
        let global = PathBuf::from(home)
            .join(".config")
            .join("ldnddev")
            .join(THEME_FILE_NAME);
        if global.exists() {
            return load_theme_file(&global, ThemeSource::Global);
        }
    }

    Ok(Theme::default())
}

fn load_theme_file(path: &Path, source: ThemeSource) -> Result<Theme> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read theme file: {}", path.display()))?;
    let version = parse_theme_version(&content)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    if version != SUPPORTED_THEME_VERSION {
        return Err(anyhow!(
            "Unsupported theme schema version `{version}` in {}; expected `{SUPPORTED_THEME_VERSION}`",
            path.display()
        ));
    }
    let colors = parse_theme_colors(&content)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    let mut header_quotes = parse_header_quotes(&content);
    if header_quotes.is_empty() {
        header_quotes = DEFAULT_HEADER_QUOTES
            .iter()
            .map(|s| s.to_string())
            .collect();
    }
    let mut theme = Theme::from_colors(colors, source, version);
    theme.header_quotes = header_quotes;
    Ok(theme)
}

fn parse_theme_version(content: &str) -> Result<u64> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "colors:" {
            break;
        }
        let Some((raw_key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        if raw_key.trim() != "version" {
            continue;
        }
        let value = extract_yaml_string_value(raw_value.trim())
            .ok_or_else(|| anyhow!("Missing value for theme key `version`"))?;
        return value
            .parse::<u64>()
            .context("Theme key `version` must be an integer");
    }

    Err(anyhow!(
        "Missing required theme key `version`; expected `{SUPPORTED_THEME_VERSION}`"
    ))
}

fn parse_theme_colors(content: &str) -> Result<ThemeColors> {
    let mut values: HashMap<String, Color> = HashMap::new();
    let mut in_colors = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "colors:" {
            in_colors = true;
            continue;
        }
        if !in_colors {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        let Some((raw_key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = raw_key.trim();
        let value = extract_yaml_string_value(raw_value.trim())
            .ok_or_else(|| anyhow!("Missing color value for theme key `{key}`"))?;
        values.insert(key.to_string(), parse_hex_color(key, value)?);
    }

    macro_rules! color {
        ($key:literal) => {
            *values
                .get($key)
                .ok_or_else(|| anyhow!("Missing required theme color `{}`", $key))?
        };
    }

    Ok(ThemeColors {
        base_background: color!("base_background"),
        body_background: color!("body_background"),
        modal_background: color!("modal_background"),
        text_primary: color!("text_primary"),
        text_secondary: color!("text_secondary"),
        text_labels: color!("text_labels"),
        text_active_focus: color!("text_active_focus"),
        modal_labels: color!("modal_labels"),
        modal_text: color!("modal_text"),
        selected_background: color!("selected_background"),
        border_default: color!("border_default"),
        border_active: color!("border_active"),
        scrollbar: color!("scrollbar"),
        scrollbar_hover: color!("scrollbar_hover"),
        input_border_default: color!("input_border_default"),
        input_border_focus: color!("input_border_focus"),
        input_text_default: color!("input_text_default"),
        input_text_focus: color!("input_text_focus"),
        cursor: color!("cursor"),
        success: color!("success"),
        warning: color!("warning"),
        error: color!("error"),
        info: color!("info"),
        folders: color!("folders"),
        files: color!("files"),
        links: color!("links"),
    })
}

fn extract_yaml_string_value(raw: &str) -> Option<&str> {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix('"') {
        return rest.split_once('"').map(|(value, _)| value);
    }
    if let Some(rest) = raw.strip_prefix('\'') {
        return rest.split_once('\'').map(|(value, _)| value);
    }
    raw.split_whitespace().next()
}

fn parse_header_quotes(content: &str) -> Vec<String> {
    let mut quotes: Vec<String> = Vec::new();
    let mut in_quotes_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "header_quotes:" {
            in_quotes_section = true;
            continue;
        }
        if in_quotes_section {
            if !line.starts_with(' ') && !line.starts_with('\t') {
                break; // end of section
            }
            // Handle YAML list items like: - "quote here"
            let item = if let Some(rest) = trimmed.strip_prefix("- ") {
                rest.trim()
            } else if let Some(rest) = trimmed.strip_prefix("-") {
                rest.trim()
            } else {
                trimmed
            };
            let value = if item.starts_with('"') || item.starts_with('\'') {
                extract_yaml_string_value(item)
            } else {
                // unquoted, take whole as value (for simplicity, assume no inline comments)
                Some(item)
            };
            if let Some(v) = value
                && !v.is_empty()
            {
                quotes.push(v.to_string());
            }
        }
    }
    quotes
}

fn parse_hex_color(key: &str, value: &str) -> Result<Color> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("Theme color `{key}` must start with #"))?;
    if hex.len() != 6 {
        return Err(anyhow!("Theme color `{key}` must be #RRGGBB"));
    }

    let r = u8::from_str_radix(&hex[0..2], 16)
        .with_context(|| format!("Invalid red channel for theme color `{key}`"))?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .with_context(|| format!("Invalid green channel for theme color `{key}`"))?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .with_context(|| format!("Invalid blue channel for theme color `{key}`"))?;
    Ok(Color::Rgb(r, g, b))
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
    pub toast: Option<Toast>,
    pub history: VecDeque<Action>,
    pub ignore_patterns: Vec<String>,
    pub theme: Theme,
    pub header_copy: String,
    pub persisted_symlinks: HashMap<String, String>,
    pub persisted_modes: HashMap<String, ActionMode>,
    pub dirty_since: Option<Instant>,
    pub persist_retry_at: Option<Instant>,
    pub last_save_error: Option<String>,
    pub theme_status: ThemeStatus,
    // Mouse support areas (updated every draw for hit-testing) and transient drag/click state
    pub last_frame_area: Rect,
    pub source_area: Rect,
    pub status_area: Rect,
    pub current_modal_area: Option<Rect>,
    pub toast_area: Option<Rect>,
    pub scrollbar_dragging: bool,
    pub last_mouse_click_pos: Option<(u16, u16, Instant)>,
    // Parallel list of source paths for the Destinations panel lines (for click-to-jump)
    pub destination_paths: Vec<PathBuf>,
}

pub const PERSIST_IDLE: Duration = Duration::from_millis(300);
pub const PERSIST_RETRY: Duration = Duration::from_secs(5);

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

    pub fn show_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast::new(level, message));
    }

    pub fn clear_expired_toast(&mut self) {
        if self.toast.as_ref().is_some_and(Toast::is_expired) {
            self.toast = None;
        }
    }

    /// Idle-since-last-mutation: every successful mutation resets the 300 ms window
    /// and drops a prior save-failure lockout so a dest-assign after fixing the
    /// path is not stuck for the rest of the 5 s. Keep last_save_error so an
    /// identical failure does not spam a new toast.
    pub fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
        self.persist_retry_at = None;
    }

    /// Snapshot maps first (so persisted_* match what save will write), then save.
    /// save() still walks state.tree; assigning persisted_* first means a later
    /// change that made save() read the maps would still be correct.
    pub fn persist_now(&mut self) -> Result<()> {
        let (dests, modes) = crate::tree::snapshot_assignments(&self.tree);
        self.persisted_symlinks = dests;
        self.persisted_modes = modes;
        let path = self.config_path.clone();
        save(self, &path)?;
        self.dirty_since = None;
        self.persist_retry_at = None;
        self.last_save_error = None;
        Ok(())
    }

    pub fn persist_now_if_dirty(&mut self) -> Result<()> {
        if self.dirty_since.is_some() {
            self.persist_now()
        } else {
            Ok(())
        }
    }

    /// Immediate flush from a key/mouse handler. Never returns Err — a disk
    /// error must not become handle_key `?` and tear down the TUI.
    pub fn persist_now_or_toast(&mut self) {
        if let Err(err) = self.persist_now() {
            self.record_persist_error(&err);
        }
    }

    pub fn record_persist_error(&mut self, err: &anyhow::Error) {
        let msg = format!("Save failed: {err}");
        self.persist_retry_at = Some(Instant::now() + PERSIST_RETRY);
        // leave dirty_since set; do not refresh an identical toast
        if self.last_save_error.as_deref() != Some(msg.as_str()) {
            self.show_toast(ToastLevel::Error, msg.clone());
            self.last_save_error = Some(msg);
        }
    }
}

pub fn load_persistent(path: &Path) -> Result<PersistentData> {
    let content = std::fs::read_to_string(path).context("Failed to read config")?;
    serde_json::from_str(&content).context("Failed to parse config")
}

pub fn load(path: &Path) -> Result<AppState> {
    let data = load_persistent(path)?;
    let root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut state = AppState::default_with_root(root);
    state.history = data.history.into_iter().collect();
    state.persisted_symlinks = data.symlinks;
    state.persisted_modes = data.modes;
    Ok(state)
}

pub fn save(state: &AppState, path: &Path) -> Result<()> {
    let (symlinks, modes) = crate::tree::collect_assignments(&state.tree);
    let data = PersistentData {
        symlinks,
        modes,
        history: state.history.iter().cloned().collect(),
    };

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(path, json)?;
    Ok(())
}
