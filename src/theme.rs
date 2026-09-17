use anyhow::Result;
use ldnddev_theme::{Palette, ParseMode, Rgb};
use ratatui::style::{Color, Modifier, Style};
use std::path::{Path, PathBuf};

pub use ldnddev_theme::{
    COLOR_FIELDS, ColorField, EditorKey, EditorOutcome, SUPPORTED_THEME_VERSION, ThemeEditor,
    ThemeEditorRow, ThemeSaveTarget, ThemeSource, default_config_home, parse_hex_input,
    theme_editor_rows,
};

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

    pub fn from_palette(palette: &Palette) -> Self {
        let mut colors = Self::default();
        for field in COLOR_FIELDS {
            if let Some(rgb) = palette.get(field.key) {
                colors.set(field.key, color_from_rgb(rgb));
            }
        }
        colors
    }

    pub fn to_palette(&self, quotes: Vec<String>, source: ThemeSource, version: u64) -> Palette {
        let mut palette = Palette::builtin();
        palette.header_quotes = quotes;
        palette.source = source;
        palette.version = version;
        for field in COLOR_FIELDS {
            if let Some(color) = self.get(field.key) {
                palette.set(field.key, color_to_rgb(color));
            }
        }
        palette
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

const DEFAULT_HEADER_QUOTES: [&str; 5] = [
    "Don't Fear the . (Dot) - Tame It.",
    ". (Dot) file Domination done right.",
    ". (Dot) file management fatigue is real. Or used to be.",
    ". (Dot) file sync setup in seconds - okay, fast.",
    ". (Dot) file management for the Ricer at heart.",
];

pub const THEME_FILE_NAME: &str = "dd_dotstore_theme.yml";

pub fn color_from_rgb(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

pub fn color_to_rgb(color: Color) -> Rgb {
    match color {
        Color::Rgb(r, g, b) => Rgb { r, g, b },
        _ => Rgb { r: 0, g: 0, b: 0 },
    }
}

pub fn color_to_hex(color: Color) -> String {
    color_to_rgb(color).to_hex()
}

pub fn color_rgb(color: Color) -> (u8, u8, u8) {
    let rgb = color_to_rgb(color);
    (rgb.r, rgb.g, rgb.b)
}

pub fn nudge_channel(color: Color, channel: usize, delta: i16) -> Color {
    color_from_rgb(color_to_rgb(color).nudge_channel(channel, delta))
}

pub fn theme_from_palette(palette: Palette) -> Theme {
    let mut theme = Theme::from_colors(
        ThemeColors::from_palette(&palette),
        palette.source,
        palette.version,
    );
    theme.header_quotes = if palette.header_quotes.is_empty() {
        DEFAULT_HEADER_QUOTES
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        palette.header_quotes
    };
    theme
}

pub fn palette_from_theme(theme: &Theme) -> Palette {
    theme
        .colors
        .to_palette(theme.header_quotes.clone(), theme.source, theme.version)
}

pub fn load_theme(project_root: &Path) -> Result<Theme> {
    let config_home = ldnddev_theme::default_config_home();
    load_theme_with(project_root, config_home.as_deref())
}

pub fn load_theme_with(project_root: &Path, config_home: Option<&Path>) -> Result<Theme> {
    let palette = ldnddev_theme::load_lookup(
        project_root,
        THEME_FILE_NAME,
        config_home,
        ParseMode::Strict,
    )?;
    Ok(theme_from_palette(palette))
}

pub fn local_theme_path(project_root: &Path) -> PathBuf {
    ldnddev_theme::local_theme_path(project_root, THEME_FILE_NAME)
}

pub fn global_theme_path(config_home: &Path) -> PathBuf {
    ldnddev_theme::global_theme_path(config_home, THEME_FILE_NAME)
}

pub fn save_theme(
    theme: &Theme,
    project_root: &Path,
    target: ThemeSaveTarget,
    config_home: Option<&Path>,
) -> Result<PathBuf> {
    let palette = palette_from_theme(theme);
    ldnddev_theme::save_theme(
        &palette,
        project_root,
        THEME_FILE_NAME,
        target,
        config_home,
        COLOR_FIELDS,
    )
}

pub fn render_theme_yaml(theme: &Theme) -> String {
    ldnddev_theme::render_yaml(&palette_from_theme(theme), COLOR_FIELDS)
}
