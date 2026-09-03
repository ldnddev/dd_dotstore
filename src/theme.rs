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
    let local = project_root.join(THEME_FILE_NAME);
    if local.exists() {
        return load_theme_file(&local, ThemeSource::Local);
    }

    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
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
    let colors = parse_theme_colors(file.colors.as_ref())
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

fn parse_theme_colors(colors: Option<&HashMap<String, String>>) -> Result<ThemeColors> {
    let raw = colors.cloned().unwrap_or_default();
    let mut values: HashMap<String, Color> = HashMap::new();
    for (key, value) in &raw {
        if value.trim().is_empty() {
            return Err(anyhow!("Missing color value for theme key `{key}`"));
        }
        values.insert(key.clone(), parse_hex_color(key, value)?);
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
