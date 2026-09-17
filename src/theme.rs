use anyhow::{Context, Result, anyhow};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

pub const THEME_FILE_NAME: &str = "dd_dotstore_theme.yml";
pub const SUPPORTED_THEME_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeSaveTarget {
    Global,
    Local,
}

impl ThemeSaveTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Local => "local",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Global => Self::Local,
            Self::Local => Self::Global,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ColorField {
    pub key: &'static str,
    pub group: &'static str,
}

pub const COLOR_FIELDS: &[ColorField] = &[
    ColorField {
        key: "base_background",
        group: "Surfaces",
    },
    ColorField {
        key: "body_background",
        group: "Surfaces",
    },
    ColorField {
        key: "modal_background",
        group: "Surfaces",
    },
    ColorField {
        key: "selected_background",
        group: "Surfaces",
    },
    ColorField {
        key: "text_primary",
        group: "Text",
    },
    ColorField {
        key: "text_secondary",
        group: "Text",
    },
    ColorField {
        key: "text_labels",
        group: "Text",
    },
    ColorField {
        key: "text_active_focus",
        group: "Text",
    },
    ColorField {
        key: "modal_labels",
        group: "Text",
    },
    ColorField {
        key: "modal_text",
        group: "Text",
    },
    ColorField {
        key: "border_default",
        group: "Chrome",
    },
    ColorField {
        key: "border_active",
        group: "Chrome",
    },
    ColorField {
        key: "scrollbar",
        group: "Chrome",
    },
    ColorField {
        key: "scrollbar_hover",
        group: "Chrome",
    },
    ColorField {
        key: "input_border_default",
        group: "Inputs",
    },
    ColorField {
        key: "input_border_focus",
        group: "Inputs",
    },
    ColorField {
        key: "input_text_default",
        group: "Inputs",
    },
    ColorField {
        key: "input_text_focus",
        group: "Inputs",
    },
    ColorField {
        key: "cursor",
        group: "Inputs",
    },
    ColorField {
        key: "success",
        group: "Status",
    },
    ColorField {
        key: "warning",
        group: "Status",
    },
    ColorField {
        key: "error",
        group: "Status",
    },
    ColorField {
        key: "info",
        group: "Status",
    },
    ColorField {
        key: "folders",
        group: "Tree",
    },
    ColorField {
        key: "files",
        group: "Tree",
    },
    ColorField {
        key: "links",
        group: "Tree",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeEditorRow {
    Header(&'static str),
    Color(usize),
}

pub fn theme_editor_rows() -> Vec<ThemeEditorRow> {
    let mut rows = Vec::new();
    let mut last_group = "";
    for (i, field) in COLOR_FIELDS.iter().enumerate() {
        if field.group != last_group {
            rows.push(ThemeEditorRow::Header(field.group));
            last_group = field.group;
        }
        rows.push(ThemeEditorRow::Color(i));
    }
    rows
}

#[derive(Clone, Debug)]
pub struct ThemeEditor {
    pub snapshot_colors: ThemeColors,
    pub snapshot_quotes: Vec<String>,
    pub snapshot_source: ThemeSource,
    pub snapshot_version: u64,
    pub selected: usize,
    pub scroll: usize,
    pub channel: usize,
    pub hex_draft: String,
    pub editing_hex: bool,
    pub save_target: ThemeSaveTarget,
}

impl ThemeEditor {
    pub fn from_theme(theme: &Theme) -> Self {
        let hex_draft = color_to_hex(
            theme
                .colors
                .get(COLOR_FIELDS[0].key)
                .unwrap_or(Color::Black),
        );
        Self {
            snapshot_colors: theme.colors,
            snapshot_quotes: theme.header_quotes.clone(),
            snapshot_source: theme.source,
            snapshot_version: theme.version,
            selected: 0,
            scroll: 0,
            channel: 0,
            hex_draft,
            editing_hex: false,
            save_target: ThemeSaveTarget::Global,
        }
    }

    pub fn selected_key(&self) -> &'static str {
        COLOR_FIELDS[self.selected.min(COLOR_FIELDS.len() - 1)].key
    }
}

const DEFAULT_HEADER_QUOTES: [&str; 5] = [
    "Don't Fear the . (Dot) - Tame It.",
    ". (Dot) file Domination done right.",
    ". (Dot) file management fatigue is real. Or used to be.",
    ". (Dot) file sync setup in seconds - okay, fast.",
    ". (Dot) file management for the Ricer at heart.",
];

/// On-disk theme schema. Fields stay optional so missing keys keep the
/// same anyhow messages the hand-rolled parser produced.
#[derive(Debug, Deserialize)]
struct ThemeFile {
    version: Option<serde_yaml::Value>,
    #[serde(default)]
    header_quotes: Vec<String>,
    colors: Option<HashMap<String, String>>,
}

pub fn load_theme(project_root: &Path) -> Result<Theme> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    load_theme_with(project_root, config_home.as_deref())
}

/// Same lookup as `load_theme`, with config-home injected so tests do not
/// mutate process-global `XDG_CONFIG_HOME`.
pub fn load_theme_with(project_root: &Path, config_home: Option<&Path>) -> Result<Theme> {
    let local = project_root.join(THEME_FILE_NAME);
    if local.exists() {
        return load_theme_file(&local, ThemeSource::Local);
    }

    if let Some(config_home) = config_home {
        let global = config_home.join("ldnddev").join(THEME_FILE_NAME);
        if global.exists() {
            return load_theme_file(&global, ThemeSource::Global);
        }
    }

    Ok(Theme::default())
}

fn load_theme_file(path: &Path, source: ThemeSource) -> Result<Theme> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read theme file: {}", path.display()))?;
    let file: ThemeFile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    let version = parse_theme_version(&file.version)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    if version != SUPPORTED_THEME_VERSION {
        return Err(anyhow!(
            "Unsupported theme schema version `{version}` in {}; expected `{SUPPORTED_THEME_VERSION}`",
            path.display()
        ));
    }
    let colors = parse_theme_colors(file.colors)
        .with_context(|| format!("Failed to parse theme file: {}", path.display()))?;
    let mut header_quotes = file.header_quotes;
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

fn parse_theme_version(version: &Option<serde_yaml::Value>) -> Result<u64> {
    let Some(value) = version else {
        return Err(anyhow!(
            "Missing required theme key `version`; expected `{SUPPORTED_THEME_VERSION}`"
        ));
    };
    match value {
        serde_yaml::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| anyhow!("Theme key `version` must be an integer")),
        serde_yaml::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Err(anyhow!("Missing value for theme key `version`"));
            }
            s.parse::<u64>()
                .context("Theme key `version` must be an integer")
        }
        serde_yaml::Value::Null => Err(anyhow!("Missing value for theme key `version`")),
        _ => Err(anyhow!("Theme key `version` must be an integer")),
    }
}

fn parse_theme_colors(colors: Option<HashMap<String, String>>) -> Result<ThemeColors> {
    let raw = colors.unwrap_or_default();
    let mut values: HashMap<String, Color> = HashMap::new();
    for (key, value) in raw {
        if value.trim().is_empty() {
            return Err(anyhow!("Missing color value for theme key `{key}`"));
        }
        let color = parse_hex_color(&key, &value)?;
        values.insert(key, color);
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

impl ThemeColors {
    pub fn get(self, key: &str) -> Option<Color> {
        Some(match key {
            "base_background" => self.base_background,
            "body_background" => self.body_background,
            "modal_background" => self.modal_background,
            "text_primary" => self.text_primary,
            "text_secondary" => self.text_secondary,
            "text_labels" => self.text_labels,
            "text_active_focus" => self.text_active_focus,
            "modal_labels" => self.modal_labels,
            "modal_text" => self.modal_text,
            "selected_background" => self.selected_background,
            "border_default" => self.border_default,
            "border_active" => self.border_active,
            "scrollbar" => self.scrollbar,
            "scrollbar_hover" => self.scrollbar_hover,
            "input_border_default" => self.input_border_default,
            "input_border_focus" => self.input_border_focus,
            "input_text_default" => self.input_text_default,
            "input_text_focus" => self.input_text_focus,
            "cursor" => self.cursor,
            "success" => self.success,
            "warning" => self.warning,
            "error" => self.error,
            "info" => self.info,
            "folders" => self.folders,
            "files" => self.files,
            "links" => self.links,
            _ => return None,
        })
    }

    pub fn set(&mut self, key: &str, color: Color) -> bool {
        let slot = match key {
            "base_background" => &mut self.base_background,
            "body_background" => &mut self.body_background,
            "modal_background" => &mut self.modal_background,
            "text_primary" => &mut self.text_primary,
            "text_secondary" => &mut self.text_secondary,
            "text_labels" => &mut self.text_labels,
            "text_active_focus" => &mut self.text_active_focus,
            "modal_labels" => &mut self.modal_labels,
            "modal_text" => &mut self.modal_text,
            "selected_background" => &mut self.selected_background,
            "border_default" => &mut self.border_default,
            "border_active" => &mut self.border_active,
            "scrollbar" => &mut self.scrollbar,
            "scrollbar_hover" => &mut self.scrollbar_hover,
            "input_border_default" => &mut self.input_border_default,
            "input_border_focus" => &mut self.input_border_focus,
            "input_text_default" => &mut self.input_text_default,
            "input_text_focus" => &mut self.input_text_focus,
            "cursor" => &mut self.cursor,
            "success" => &mut self.success,
            "warning" => &mut self.warning,
            "error" => &mut self.error,
            "info" => &mut self.info,
            "folders" => &mut self.folders,
            "files" => &mut self.files,
            "links" => &mut self.links,
            _ => return false,
        };
        *slot = color;
        true
    }
}

pub fn color_to_hex(color: Color) -> String {
    let (r, g, b) = color_rgb(color);
    format!("#{r:02X}{g:02X}{b:02X}")
}

pub fn color_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

pub fn parse_hex_input(value: &str) -> Result<Color> {
    let trimmed = value.trim();
    let with_hash = if trimmed.starts_with('#') {
        trimmed.to_string()
    } else {
        format!("#{trimmed}")
    };
    parse_hex_color("color", &with_hash)
}

pub fn nudge_channel(color: Color, channel: usize, delta: i16) -> Color {
    let (mut r, mut g, mut b) = color_rgb(color);
    let slot = match channel % 3 {
        0 => &mut r,
        1 => &mut g,
        _ => &mut b,
    };
    let next = i16::from(*slot) + delta;
    *slot = next.clamp(0, 255) as u8;
    Color::Rgb(r, g, b)
}

pub fn default_config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

pub fn local_theme_path(project_root: &Path) -> PathBuf {
    project_root.join(THEME_FILE_NAME)
}

pub fn global_theme_path(config_home: &Path) -> PathBuf {
    config_home.join("ldnddev").join(THEME_FILE_NAME)
}

pub fn save_theme(
    theme: &Theme,
    project_root: &Path,
    target: ThemeSaveTarget,
    config_home: Option<&Path>,
) -> Result<PathBuf> {
    let path = match target {
        ThemeSaveTarget::Local => local_theme_path(project_root),
        ThemeSaveTarget::Global => {
            let home = config_home
                .map(Path::to_path_buf)
                .or_else(default_config_home)
                .ok_or_else(|| {
                    anyhow!("Cannot save global theme: XDG_CONFIG_HOME and HOME are unset")
                })?;
            global_theme_path(&home)
        }
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create theme directory {}", parent.display()))?;
    }
    fs::write(&path, render_theme_yaml(theme))
        .with_context(|| format!("Failed to write theme file {}", path.display()))?;
    Ok(path)
}

pub fn render_theme_yaml(theme: &Theme) -> String {
    let mut out = String::from("version: 1\n");
    if !theme.header_quotes.is_empty() {
        out.push_str("header_quotes:\n");
        for quote in &theme.header_quotes {
            out.push_str("  - ");
            out.push_str(&yaml_quote(quote));
            out.push('\n');
        }
    }
    out.push_str("colors:\n");
    let mut last_group = "";
    for field in COLOR_FIELDS {
        if field.group != last_group {
            out.push_str("\n  # ");
            out.push_str(field.group);
            out.push('\n');
            last_group = field.group;
        }
        let hex = theme
            .colors
            .get(field.key)
            .map(color_to_hex)
            .unwrap_or_else(|| "#000000".to_string());
        out.push_str("  ");
        out.push_str(field.key);
        out.push_str(": \"");
        out.push_str(&hex);
        out.push_str("\"\n");
    }
    out
}

fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
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
