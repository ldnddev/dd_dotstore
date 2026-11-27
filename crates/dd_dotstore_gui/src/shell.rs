//! One shell. The tree is the terminal `App`, not a second model.
//! This crate does not import ratatui or crossterm.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dd_dotstore::app::App;
use dd_dotstore::commands::{self, Command, InputMode, Surface};
use dd_dotstore::domain::{Modal, NodeKind};
use dd_dotstore::tree::{self, flatten_visible};
use gpui::{
    ClickEvent, Context, FocusHandle, Focusable, FontWeight, Hsla, IntoElement, KeyDownEvent,
    ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, px, relative,
};
use ldnddev_theme::{
    EditorKey, EditorOutcome, Palette, PickerOutcome, PickerRow, ThemeEditor, ThemePicker,
    ThemeSaveTarget,
};

use crate::present::{self, ModalView};
#[cfg(test)]
use crate::settings::Density;
use crate::settings::{self, GuiSettings};

const CHROME: f32 = 28.0;
const NAV: [&str; 4] = ["Themes", "Window", "About", "Help"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Themes = 0,
    Window = 1,
    About = 2,
    Help = 3,
}

impl Page {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Window,
            2 => Self::About,
            3 => Self::Help,
            _ => Self::Themes,
        }
    }

    fn label(self) -> &'static str {
        NAV[self as usize]
    }
}

struct Sheet {
    picker: ThemePicker,
    editor: Option<ThemeEditor>,
    /// No Omarchy names, so the color editor is the Themes page.
    direct_editor: bool,
    snapshot: Palette,
    page: Page,
    row: usize,
    query: String,
    query_on: bool,
    hex_error: Option<String>,
}

pub struct ShellOptions {
    pub root: Option<PathBuf>,
    pub settings_file: PathBuf,
    pub quit_runtime: bool,
}

impl ShellOptions {
    pub fn production(root: Option<PathBuf>) -> Self {
        Self {
            root,
            settings_file: settings::settings_path(),
            quit_runtime: true,
        }
    }
}

pub struct Shell {
    pub(crate) app: Option<App>,
    load_error: Option<String>,
    settings: GuiSettings,
    settings_file: PathBuf,
    sheet: Option<Sheet>,
    filter_on: bool,
    caret: usize,
    notice: Option<String>,
    focus: FocusHandle,
    tree_scroll: ScrollHandle,
    dest_scroll: ScrollHandle,
    settings_scroll: ScrollHandle,
    /// Saved source path last scrolled into Destinations.
    dest_follow: Option<PathBuf>,
    last_poll: Instant,
    quit_runtime: bool,
    pub quit: bool,
}

impl Shell {
    pub fn open(options: ShellOptions, cx: &mut Context<Self>) -> Self {
        let settings = settings::load_from(&options.settings_file);
        let (app, load_error) = match &options.root {
            Some(root) => match App::new_with_root(root) {
                Ok(app) => (Some(app), None),
                Err(err) => (None, Some(format!("{err:#}"))),
            },
            None => match App::new() {
                Ok(app) => (Some(app), None),
                Err(err) => (None, Some(format!("{err:#}"))),
            },
        };
        Self {
            app,
            load_error,
            settings,
            settings_file: options.settings_file,
            sheet: None,
            filter_on: false,
            caret: 0,
            notice: None,
            focus: cx.focus_handle(),
            tree_scroll: ScrollHandle::new(),
            dest_scroll: ScrollHandle::new(),
            settings_scroll: ScrollHandle::new(),
            dest_follow: None,
            last_poll: Instant::now(),
            quit_runtime: options.quit_runtime,
            quit: false,
        }
    }

    pub fn handle_key(&mut self, key: &str) {
        let key = key.to_ascii_lowercase();
        if self.command(&key) == Some(Command::Quit) {
            self.quit = true;
            self.sync_view();
            return;
        }
        // A dialog stays above Settings, matching the terminal. F1 and F3 still open the sheet.
        if self.modal_open() {
            if self.command(&key) == Some(Command::Settings) {
                self.toggle_sheet(Page::Themes);
            } else if self.command(&key) == Some(Command::About) {
                self.show_about();
            } else {
                self.forward(&key);
            }
            self.sync_view();
            return;
        }
        if self.sheet.is_some() {
            self.sheet_key(&key);
            self.sync_view();
            return;
        }
        if self.command(&key) == Some(Command::Settings) {
            self.open_sheet(Page::Themes);
            self.sync_view();
            return;
        }
        if self.command(&key) == Some(Command::About) {
            self.open_sheet(Page::About);
            self.sync_view();
            return;
        }
        if self.filter_on {
            self.filter_key(&key);
            self.sync_view();
            return;
        }
        self.tree_key(&key);
        self.sync_view();
    }

    pub fn paste(&mut self, raw: &str) {
        if self.modal_open() && !self.editing_hex() {
            if let Some(app) = self.app.as_mut() {
                if let Err(err) = app.paste_text(raw) {
                    self.notice = Some(format!("{err:#}"));
                }
            }
            return;
        }
        let graphic: String = raw
            .chars()
            .filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
            .collect();
        if graphic.is_empty() {
            return;
        }
        if self.editing_hex() {
            for ch in graphic.chars() {
                self.editor_key(&ch.to_string());
            }
            return;
        }
        if self.filter_on {
            self.filter_insert(&graphic);
        }
    }

    #[cfg(test)]
    pub fn visible_text(&self) -> String {
        self.lines().join("\n")
    }

    #[cfg(test)]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("dd_dotstore".to_string());
        lines.push(format!("quote:{}", self.header_quote()));
        lines.push(format!("density:{}", density_token(self.settings.density)));
        lines.push(self.focus_dump());
        if let Some(err) = &self.load_error {
            lines.push(err.clone());
        }
        lines.push(format!("filter:{}", self.filter_text()));
        lines.extend(self.tree_lines());
        let dests = self.destination_lines();
        lines.push(format!("destinations:{}", dests.len()));
        lines.extend(dests);
        if let Some(toast) = self.toast_dump() {
            lines.push(toast);
        }
        if let Some(view) = self.modal_dump() {
            lines.extend(present::modal_dump(&view));
        }
        lines.push(self.footer_line().to_string());
        if let Some(notice) = &self.notice {
            lines.push(notice.clone());
        }
        if let Some(sheet) = &self.sheet {
            lines.push("settings-panel:right:70".to_string());
            lines.push("settings-nav:Themes,Window,About,Help".to_string());
            lines.push(format!("settings-page:{}", self.page_label(sheet)));
            lines.push("Settings".to_string());
            for (index, label) in NAV.iter().enumerate() {
                let on = sheet.query.trim().is_empty() && sheet.page as usize == index;
                let mark = if on { ">" } else { " " };
                lines.push(format!("{mark} {label}"));
            }
            if !sheet.query.trim().is_empty() {
                lines.push("Results".to_string());
                lines.extend(self.result_labels());
            } else {
                match sheet.page {
                    Page::Themes => lines.extend(self.theme_lines(sheet)),
                    Page::Window => lines.extend(self.window_lines()),
                    Page::About => lines.push(self.about_text()),
                    Page::Help => lines.extend(self.help_lines()),
                }
            }
        }
        lines
    }

    fn command(&self, key: &str) -> Option<Command> {
        commands::lookup(
            key,
            Surface::Gui,
            InputMode::Kitty,
            self.settings.vim_keys,
            self.in_text(),
        )
    }

    fn in_text(&self) -> bool {
        self.filter_on
            || self.sheet.as_ref().is_some_and(|sheet| {
                sheet.query_on
                    || sheet
                        .editor
                        .as_ref()
                        .is_some_and(|editor| editor.editing_hex)
            })
    }

    fn footer_line(&self) -> &'static str {
        if self.sheet.is_some() || self.modal_open() {
            "F1: Settings  Esc: Close  Ctrl+Q: Quit"
        } else {
            "F1: Settings  j/k: Move  Space: Select  s: Apply  /: Filter  Ctrl+Q: Quit"
        }
    }

    fn filter_text(&self) -> String {
        self.app
            .as_ref()
            .map(|app| app.state.filter.clone())
            .unwrap_or_default()
    }

    fn source_paints(&self) -> Vec<SourcePaint> {
        let Some(app) = &self.app else {
            return Vec::new();
        };
        let selected = app.state.list_state.selected();
        app.state
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let mut line = present::source_line(node, selected == Some(index));
                line.push_str(&present::suggestion_suffix(&app.state, node));
                SourcePaint {
                    line,
                    check: if node.selected { "[✓]" } else { "[ ]" },
                    folder: present::row_has_children(node),
                    expanded: present::folder_expanded(node),
                }
            })
            .collect()
    }

    #[cfg(test)]
    fn tree_lines(&self) -> Vec<String> {
        let Some(app) = &self.app else {
            return Vec::new();
        };
        let selected = app.state.list_state.selected();
        app.state
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let mut line = present::source_line(node, selected == Some(index));
                line.push_str(&present::suggestion_suffix(&app.state, node));
                line
            })
            .collect()
    }

    fn destination_lines(&self) -> Vec<String> {
        self.app
            .as_ref()
            .map(|app| {
                present::saved_destinations(&app.state)
                    .into_iter()
                    .map(|(line, _)| line)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn focus_dump(&self) -> String {
        self.app
            .as_ref()
            .map(|app| present::focus_line(&app.state))
            .unwrap_or_else(|| "focus:".to_string())
    }

    fn header_quote(&self) -> String {
        self.app
            .as_ref()
            .map(|app| app.state.header_copy.clone())
            .unwrap_or_default()
    }

    fn toast_dump(&self) -> Option<String> {
        self.app
            .as_ref()
            .and_then(|app| app.state.toast.as_ref().map(present::toast_line))
    }

    fn modal_dump(&self) -> Option<ModalView> {
        self.app
            .as_ref()
            .and_then(|app| present::modal_view(&app.state))
    }

    fn modal_open(&self) -> bool {
        self.app
            .as_ref()
            .is_some_and(|app| app.state.modal.is_some())
    }

    fn page_label(&self, sheet: &Sheet) -> String {
        if sheet.query.trim().is_empty() {
            sheet.page.label().to_string()
        } else {
            "Results".to_string()
        }
    }

    #[cfg(test)]
    fn theme_lines(&self, sheet: &Sheet) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(app) = &self.app {
            lines.push(format!("Source: {}", app.state.theme.source.label()));
            lines.push(format!("Status: {}", app.state.theme_status.message));
        }
        let target = sheet
            .editor
            .as_ref()
            .map(|editor| editor.save_target)
            .unwrap_or(sheet.picker.save_target);
        lines.push(format!("Save to {}", target.label()));
        if let Some(editor) = &sheet.editor {
            lines.push("Theme color editor".to_string());
            lines.push(
                if sheet.direct_editor {
                    "Revert"
                } else {
                    "Back"
                }
                .to_string(),
            );
            for (index, field) in editor.fields.iter().enumerate() {
                let hex = editor
                    .palette
                    .get(field.key)
                    .map(|color| color.to_hex())
                    .unwrap_or_else(|| "#000000".to_string());
                let mark = if index == editor.selected { ">" } else { " " };
                lines.push(format!("{mark} {} {hex}", field.key));
            }
            lines.push(format!("Hex: {}", editor.hex_draft));
            if let Some(err) = &sheet.hex_error {
                lines.push(err.clone());
            }
        } else {
            lines.push("theme-list:flow".to_string());
            for (index, row) in sheet.picker.rows.iter().enumerate() {
                let mark = if index == sheet.picker.selected {
                    ">"
                } else {
                    " "
                };
                lines.push(format!("{mark} {}", theme_row_label(row)));
            }
            lines.push("Edit colors".to_string());
        }
        lines.push("Save".to_string());
        lines
    }

    #[cfg(test)]
    fn window_lines(&self) -> Vec<String> {
        vec![
            format!("Density {}", self.settings.density.label()),
            format!(
                "Vim keys {}",
                if self.settings.vim_keys { "on" } else { "off" }
            ),
            format!(
                "Reduced motion {}",
                if self.settings.reduced_motion {
                    "on"
                } else {
                    "off"
                }
            ),
            "Comfortable".to_string(),
            "Compact".to_string(),
            "Vim".to_string(),
        ]
    }

    fn about_text(&self) -> String {
        match &self.app {
            Some(app) => dd_dotstore::ui::settings::about_copy(
                app.state.theme.source.label(),
                &app.state.theme_status.message,
            ),
            None => dd_dotstore::ui::settings::about_copy("default", "not loaded"),
        }
    }

    #[cfg(test)]
    fn help_lines(&self) -> Vec<String> {
        commands::shortcut_sheet()
            .iter()
            .map(commands::shortcut_line)
            .collect()
    }

    fn result_labels(&self) -> Vec<String> {
        let Some(sheet) = &self.sheet else {
            return Vec::new();
        };
        let query = sheet.query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for row in &sheet.picker.rows {
            let label = theme_row_label(row);
            if label.to_ascii_lowercase().contains(&query) {
                hits.push(label);
            }
        }
        for label in ["Density", "Vim keys", "Reduced motion"] {
            if label.to_ascii_lowercase().contains(&query) {
                hits.push(label.to_string());
            }
        }
        for row in commands::shortcut_sheet() {
            if row.action.to_ascii_lowercase().contains(&query)
                && !hits.iter().any(|hit| hit == row.action)
            {
                hits.push(row.action.to_string());
            }
        }
        hits
    }

    fn sheet_key(&mut self, key: &str) {
        if self.command(key) == Some(Command::Settings) {
            self.close_sheet(true);
            return;
        }
        if self.command(key) == Some(Command::About) {
            if let Some(sheet) = self.sheet.as_mut() {
                sheet.page = Page::About;
                sheet.row = 0;
                sheet.query.clear();
                sheet.query_on = false;
            }
            return;
        }
        if self.sheet.as_ref().is_some_and(|sheet| sheet.query_on) {
            self.search_key(key);
            return;
        }
        if key == "/" && !self.editing_hex() {
            if let Some(sheet) = self.sheet.as_mut() {
                sheet.query_on = true;
                sheet.query.clear();
                sheet.row = 0;
            }
            return;
        }
        let hex = self.editing_hex();
        if let Some(dir) = section_dir(key, hex) {
            self.jump_page(dir);
            return;
        }
        let page = self.sheet.as_ref().map(|sheet| sheet.page);
        match page {
            Some(Page::Themes) => self.themes_key(key),
            Some(Page::Window) => self.window_key(key),
            Some(Page::About | Page::Help) if key == "escape" => self.close_sheet(true),
            Some(Page::About | Page::Help) | None => {}
        }
    }

    fn themes_key(&mut self, key: &str) {
        if key == "y" || key == "s" {
            self.save_theme();
            return;
        }
        if key == "t" && !self.editing_hex() {
            self.toggle_target();
            return;
        }
        if self
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.editor.is_some())
        {
            self.editor_key(key);
            return;
        }
        self.theme_list_key(key);
    }

    fn theme_list_key(&mut self, key: &str) {
        let vim = self.settings.vim_keys;
        match key {
            "escape" => self.close_sheet(true),
            "enter" => self.preview_after_handle(EditorKey::Enter),
            "e" => self.open_editor(),
            "up" => self.preview_after_handle(EditorKey::Up),
            "down" => self.preview_after_handle(EditorKey::Down),
            "pageup" => self.page_theme(-8),
            "pagedown" => self.page_theme(8),
            "k" if vim => self.preview_after_handle(EditorKey::Char('k')),
            "j" if vim => self.preview_after_handle(EditorKey::Char('j')),
            "g" if vim => self.preview_after_handle(EditorKey::Char('g')),
            "shift-g" if vim => self.preview_after_handle(EditorKey::Char('G')),
            _ => {}
        }
    }

    fn preview_after_handle(&mut self, key: EditorKey) {
        let outcome = self.sheet.as_mut().map(|sheet| sheet.picker.handle(key));
        let Some(outcome) = outcome else {
            return;
        };
        if matches!(outcome, PickerOutcome::None) {
            let preview = self
                .sheet
                .as_mut()
                .map(|sheet| sheet.picker.preview_selected());
            if let Some(preview) = preview {
                self.finish_picker(preview);
                if let Some(index) = self.sheet.as_ref().map(|sheet| sheet.picker.selected) {
                    self.settings_scroll.scroll_to_item(index);
                }
            }
            return;
        }
        self.finish_picker(outcome);
    }

    fn page_theme(&mut self, delta: isize) {
        let next = {
            let Some(sheet) = self.sheet.as_mut() else {
                return;
            };
            let len = sheet.picker.rows.len() as isize;
            if len == 0 {
                return;
            }
            let next = (sheet.picker.selected as isize + delta).clamp(0, len - 1) as usize;
            sheet.picker.select(next);
            next
        };
        let preview = self
            .sheet
            .as_mut()
            .map(|sheet| sheet.picker.preview_selected());
        if let Some(preview) = preview {
            self.finish_picker(preview);
        }
        self.settings_scroll.scroll_to_item(next);
    }

    fn finish_picker(&mut self, outcome: PickerOutcome) {
        match outcome {
            PickerOutcome::Applied => {
                let palette = self
                    .sheet
                    .as_ref()
                    .map(|sheet| sheet.picker.palette.clone());
                if let Some(palette) = palette {
                    self.apply_palette(palette);
                }
            }
            PickerOutcome::EditColors => self.open_editor(),
            PickerOutcome::RequestSave => self.save_theme(),
            PickerOutcome::Error(message) => self.notice = Some(message),
            PickerOutcome::Closed { .. } => self.close_sheet(true),
            PickerOutcome::None => {}
        }
    }

    fn open_editor(&mut self) {
        let Some(sheet) = self.sheet.as_mut() else {
            return;
        };
        if sheet.editor.is_some() {
            return;
        }
        let mut editor = ThemeEditor::new(
            sheet.picker.palette.clone(),
            dd_dotstore::theme::extra_theme_fields(),
        );
        editor.save_target = sheet.picker.save_target;
        sheet.editor = Some(editor);
        sheet.hex_error = None;
    }

    fn editor_key(&mut self, key: &str) {
        if key == "escape" {
            self.editor_escape();
            return;
        }
        let hex = self.editing_hex();
        let vim = self.settings.vim_keys;
        let ek = match key {
            "up" => Some(EditorKey::Up),
            "down" => Some(EditorKey::Down),
            "left" => Some(EditorKey::Left),
            "right" => Some(EditorKey::Right),
            "enter" => Some(EditorKey::Enter),
            "backspace" => Some(EditorKey::Backspace),
            "j" | "k" | "g" | "shift-g" if !hex && !vim => None,
            other => typed_char(other).map(EditorKey::Char),
        };
        let Some(ek) = ek else {
            return;
        };
        let shift = key.starts_with("shift-");
        let outcome = self
            .sheet
            .as_mut()
            .and_then(|sheet| sheet.editor.as_mut())
            .map(|editor| editor.handle(ek, shift));
        match outcome {
            Some(EditorOutcome::PaletteChanged) => {
                if let Some(sheet) = self.sheet.as_mut() {
                    sheet.hex_error = None;
                }
                self.apply_editor_palette();
            }
            Some(EditorOutcome::RequestSave) => self.save_theme(),
            Some(EditorOutcome::Closed { .. }) => self.editor_escape(),
            Some(EditorOutcome::HexError(err)) => {
                if let Some(sheet) = self.sheet.as_mut() {
                    sheet.hex_error = Some(err);
                }
            }
            Some(EditorOutcome::None) | None => {}
        }
    }

    fn editor_escape(&mut self) {
        if self.editing_hex() {
            if let Some(editor) = self.sheet.as_mut().and_then(|sheet| sheet.editor.as_mut()) {
                let _ = editor.handle(EditorKey::Esc, false);
            }
            return;
        }
        let step_back = self.sheet.as_ref().is_some_and(|sheet| {
            sheet.editor.is_some() && sheet.picker.lists_omarchy() && !sheet.direct_editor
        });
        if step_back {
            let palette = {
                let sheet = self.sheet.as_mut().expect("sheet");
                let editor = sheet.editor.take().expect("editor");
                sheet.picker.palette = editor.palette.clone();
                sheet.picker.save_target = editor.save_target;
                sheet.hex_error = None;
                editor.palette
            };
            self.apply_palette(palette);
            return;
        }
        self.close_sheet(true);
    }

    fn window_key(&mut self, key: &str) {
        let vim = self.settings.vim_keys;
        let last = 2usize;
        match key {
            "escape" => self.close_sheet(true),
            "up" => self.bump_row(-1, last),
            "down" => self.bump_row(1, last),
            "k" if vim => self.bump_row(-1, last),
            "j" if vim => self.bump_row(1, last),
            "enter" => self.activate_window(),
            _ => {}
        }
    }

    fn activate_window(&mut self) {
        let row = self.sheet.as_ref().map(|sheet| sheet.row).unwrap_or(0);
        match row {
            0 => self.settings.density = self.settings.density.toggled(),
            1 => self.settings.vim_keys = !self.settings.vim_keys,
            _ => self.settings.reduced_motion = !self.settings.reduced_motion,
        }
        self.persist_settings();
    }

    fn bump_row(&mut self, delta: isize, last: usize) {
        let Some(sheet) = self.sheet.as_mut() else {
            return;
        };
        let next = sheet.row as isize + delta;
        sheet.row = next.clamp(0, last as isize) as usize;
    }

    fn search_key(&mut self, key: &str) {
        match key {
            "escape" => {
                if let Some(sheet) = self.sheet.as_mut() {
                    sheet.query_on = false;
                    sheet.query.clear();
                    sheet.row = 0;
                }
            }
            "backspace" => {
                if let Some(sheet) = self.sheet.as_mut() {
                    sheet.query.pop();
                    sheet.row = 0;
                }
            }
            "enter" => self.activate_result(),
            "up" => self.bump_row(-1, self.result_labels().len().saturating_sub(1)),
            "down" => self.bump_row(1, self.result_labels().len().saturating_sub(1)),
            "k" if self.settings.vim_keys => {
                self.bump_row(-1, self.result_labels().len().saturating_sub(1));
            }
            "j" if self.settings.vim_keys => {
                self.bump_row(1, self.result_labels().len().saturating_sub(1));
            }
            other => {
                if let Some(ch) = typed_char(other) {
                    if let Some(sheet) = self.sheet.as_mut() {
                        sheet.query.push(ch);
                        sheet.row = 0;
                    }
                }
            }
        }
    }

    fn activate_result(&mut self) {
        let labels = self.result_labels();
        let row = self.sheet.as_ref().map(|sheet| sheet.row).unwrap_or(0);
        let Some(label) = labels.get(row).cloned() else {
            return;
        };
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.query.clear();
            sheet.query_on = false;
            sheet.row = 0;
        }
        if label == "Density" || label == "Vim keys" || label == "Reduced motion" {
            if let Some(sheet) = self.sheet.as_mut() {
                sheet.page = Page::Window;
            }
            return;
        }
        if let Some(index) = self.sheet.as_ref().and_then(|sheet| {
            sheet
                .picker
                .rows
                .iter()
                .position(|row| theme_row_label(row) == label)
        }) {
            if let Some(sheet) = self.sheet.as_mut() {
                sheet.page = Page::Themes;
                sheet.picker.select(index);
            }
            let preview = self
                .sheet
                .as_mut()
                .map(|sheet| sheet.picker.preview_selected());
            if let Some(preview) = preview {
                self.finish_picker(preview);
            }
            return;
        }
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.page = Page::Help;
        }
    }

    fn jump_page(&mut self, dir: i32) {
        let Some(sheet) = self.sheet.as_mut() else {
            return;
        };
        let next = (sheet.page as i32 + dir).clamp(0, (NAV.len() - 1) as i32) as u8;
        sheet.page = Page::from_u8(next);
        sheet.row = 0;
    }

    fn open_sheet(&mut self, page: Page) {
        if self.sheet.is_some() {
            return;
        }
        self.filter_on = false;
        if let Some(app) = self.app.as_mut() {
            app.state.filter_editing = false;
        }
        let palette = self.current_palette();
        let paths = self.omarchy_paths();
        let picker = ThemePicker::new(
            palette.clone(),
            dd_dotstore::theme::extra_theme_fields(),
            paths,
        );
        let direct = !picker.lists_omarchy();
        let mut sheet = Sheet {
            picker,
            editor: None,
            direct_editor: direct,
            snapshot: palette,
            page,
            row: 0,
            query: String::new(),
            query_on: false,
            hex_error: None,
        };
        if direct {
            let mut editor = ThemeEditor::new(
                sheet.picker.palette.clone(),
                dd_dotstore::theme::extra_theme_fields(),
            );
            editor.save_target = sheet.picker.save_target;
            sheet.editor = Some(editor);
        }
        self.sheet = Some(sheet);
    }

    fn close_sheet(&mut self, revert: bool) {
        let Some(sheet) = self.sheet.take() else {
            return;
        };
        if revert {
            self.apply_palette(sheet.snapshot);
        }
    }

    fn toggle_target(&mut self) {
        let Some(sheet) = self.sheet.as_mut() else {
            return;
        };
        if let Some(editor) = sheet.editor.as_mut() {
            editor.save_target = editor.save_target.toggled();
            sheet.picker.save_target = editor.save_target;
            return;
        }
        sheet.picker.save_target = sheet.picker.save_target.toggled();
    }

    fn save_theme(&mut self) {
        let Some(sheet) = &self.sheet else {
            return;
        };
        let palette = sheet
            .editor
            .as_ref()
            .map(|editor| editor.palette.clone())
            .unwrap_or_else(|| sheet.picker.palette.clone());
        let target = sheet
            .editor
            .as_ref()
            .map(|editor| editor.save_target)
            .unwrap_or(sheet.picker.save_target);
        let fields = sheet
            .editor
            .as_ref()
            .map(|editor| editor.fields.clone())
            .unwrap_or_else(|| {
                let mut fields = ldnddev_theme::COLOR_FIELDS.to_vec();
                for extra in dd_dotstore::theme::extra_theme_fields() {
                    if !fields.iter().any(|field| field.key == extra.key) {
                        fields.push(*extra);
                    }
                }
                fields
            });
        let root = self
            .app
            .as_ref()
            .map(|app| app.state.project_root.clone())
            .unwrap_or_default();
        match ldnddev_theme::save_theme(
            &palette,
            &root,
            dd_dotstore::theme::THEME_FILE_NAME,
            target,
            ldnddev_theme::default_config_home().as_deref(),
            &fields,
        ) {
            Ok(path) => {
                self.apply_palette(palette);
                if let Some(app) = self.app.as_mut() {
                    let source = match target {
                        ThemeSaveTarget::Local => ldnddev_theme::ThemeSource::Local,
                        ThemeSaveTarget::Global => ldnddev_theme::ThemeSource::Global,
                    };
                    app.state.theme.source = source;
                    app.state.theme_status =
                        dd_dotstore::theme::ThemeStatus::healthy(source, app.state.theme.version);
                }
                self.notice = Some(format!(
                    "Saved {} theme to {}",
                    target.label(),
                    path.display()
                ));
                self.sheet = None;
            }
            Err(err) => self.notice = Some(format!("Save failed: {err}")),
        }
    }

    fn apply_editor_palette(&mut self) {
        let palette = self
            .sheet
            .as_ref()
            .and_then(|sheet| sheet.editor.as_ref().map(|editor| editor.palette.clone()));
        let Some(palette) = palette else {
            return;
        };
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.picker.palette = palette.clone();
        }
        self.apply_palette(palette);
    }

    fn apply_palette(&mut self, palette: Palette) {
        if let Some(app) = self.app.as_mut() {
            app.state.theme = dd_dotstore::theme::theme_from_palette(palette);
        }
    }

    fn current_palette(&self) -> Palette {
        self.app
            .as_ref()
            .map(|app| dd_dotstore::theme::palette_from_theme(&app.state.theme))
            .unwrap_or_else(Palette::builtin)
    }

    fn omarchy_paths(&self) -> ldnddev_theme::OmarchyPaths {
        self.app
            .as_ref()
            .map(|app| app.state.omarchy_paths.clone())
            .unwrap_or_else(ldnddev_theme::OmarchyPaths::system)
    }

    fn editing_hex(&self) -> bool {
        self.sheet
            .as_ref()
            .and_then(|sheet| sheet.editor.as_ref())
            .is_some_and(|editor| editor.editing_hex)
    }

    fn persist_settings(&mut self) {
        if let Err(err) = settings::save_to(&self.settings_file, &self.settings) {
            self.notice = Some(format!("Could not save gui.json: {err}"));
        }
    }

    fn tree_key(&mut self, key: &str) {
        let vim = self.settings.vim_keys;
        let command = matches!(
            key,
            "up" | "down"
                | "left"
                | "right"
                | "space"
                | "enter"
                | "e"
                | "m"
                | "shift-m"
                | "s"
                | "p"
                | "x"
                | "u"
                | "r"
                | "i"
                | "shift-i"
                | "shift-e"
                | "t"
                | "shift-t"
                | "shift-a"
                | "shift-d"
                | "ctrl-g"
                | "pageup"
                | "pagedown"
        );
        let vim_motion = vim && matches!(key, "j" | "k" | "h" | "l" | "g" | "shift-g");
        match key {
            "/" => self.begin_filter(),
            "escape" => {
                if self.filter_text().is_empty() {
                    self.forward("escape");
                } else {
                    self.clear_filter();
                }
            }
            _ if command || vim_motion => self.forward(key),
            _ => {}
        }
    }

    fn forward(&mut self, key: &str) {
        let Some(app) = self.app.as_mut() else {
            return;
        };
        match dd_dotstore::input::handle_token(&mut app.state, key) {
            Ok(true) => self.quit = true,
            Ok(false) => {}
            Err(err) => self.notice = Some(format!("{err:#}")),
        }
    }

    fn toggle_sheet(&mut self, page: Page) {
        if self.sheet.is_some() {
            self.close_sheet(true);
        } else {
            self.open_sheet(page);
        }
    }

    fn show_about(&mut self) {
        if self.sheet.is_none() {
            self.open_sheet(Page::About);
            return;
        }
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.page = Page::About;
            sheet.row = 0;
            sheet.query.clear();
            sheet.query_on = false;
        }
    }

    fn sync_view(&mut self) {
        let Some(app) = self.app.as_ref() else {
            return;
        };
        if let Some(index) = app.state.list_state.selected() {
            self.tree_scroll.scroll_to_item(index);
        }
        let saved = app
            .state
            .list_state
            .selected()
            .and_then(|index| app.state.nodes.get(index))
            .filter(|node| node.assigned_dest().is_some())
            .map(|node| node.path.clone());
        if saved == self.dest_follow {
            return;
        }
        self.dest_follow = saved.clone();
        if let Some(path) = saved {
            if let Some(index) = present::saved_destinations(&app.state)
                .iter()
                .position(|(_, source)| source == &path)
            {
                self.dest_scroll.scroll_to_item(index);
            }
        }
    }

    /// Highlight a source row. Shift checks the range. A double-click opens the browser.
    pub fn click_source(&mut self, index: usize, shift: bool, clicks: usize) {
        let open_browser = {
            let Some(app) = self.app.as_mut() else {
                return;
            };
            if index >= app.state.nodes.len() {
                return;
            }
            let prev = app.state.list_state.selected();
            app.state.list_state.select(Some(index));
            if shift {
                if let Some(cur) = prev {
                    let start = cur.min(index);
                    let end = cur.max(index);
                    for row in start..=end {
                        if let Some(node) = app.state.nodes.get(row) {
                            let path = node.path.clone();
                            if let Some(node) = tree::find_mut_node(&mut app.state.tree, &path) {
                                node.selected = true;
                            }
                        }
                    }
                    flatten_visible(&mut app.state);
                    let last = app.state.nodes.len().saturating_sub(1);
                    app.state.list_state.select(Some(index.min(last)));
                }
                false
            } else {
                clicks >= 2
            }
        };
        if open_browser {
            self.forward("e");
        }
        self.sync_view();
    }

    pub fn click_checkbox(&mut self, index: usize) {
        self.click_source(index, false, 1);
        self.forward("space");
        self.sync_view();
    }

    pub fn click_twist(&mut self, index: usize) {
        {
            let Some(app) = self.app.as_mut() else {
                return;
            };
            let Some(path) = app.state.nodes.get(index).map(|node| node.path.clone()) else {
                return;
            };
            if !matches!(
                app.state.nodes.get(index).map(|node| &node.kind),
                Some(NodeKind::Folder { .. })
            ) {
                return;
            }
            tree::toggle_expand(&mut app.state, &path);
            flatten_visible(&mut app.state);
            let last = app.state.nodes.len().saturating_sub(1);
            app.state.list_state.select(Some(index.min(last)));
        }
        self.sync_view();
    }

    pub fn click_destination(&mut self, index: usize) {
        {
            let Some(app) = self.app.as_mut() else {
                return;
            };
            let Some(path) = present::saved_destinations(&app.state)
                .get(index)
                .map(|(_, path)| path.clone())
            else {
                return;
            };
            tree::expand_ancestors(&mut app.state, &path);
            flatten_visible(&mut app.state);
            if let Some(row) = app.state.nodes.iter().position(|node| node.path == path) {
                app.state.list_state.select(Some(row));
            }
        }
        self.sync_view();
    }

    pub fn click_modal_row(&mut self, index: usize, clicks: usize) {
        let confirm = {
            let Some(app) = self.app.as_mut() else {
                return;
            };
            match &mut app.state.modal {
                Some(Modal::EditDest { browser, .. }) => browser.selected = index,
                Some(Modal::IgnoreEditor { selected, .. }) => *selected = index,
                Some(Modal::ImportPicker { selected, .. }) => *selected = index,
                Some(Modal::Adopt { selected, .. }) => *selected = index,
                Some(Modal::Doctor {
                    selected, scroll, ..
                }) => {
                    *selected = index;
                    if index < *scroll {
                        *scroll = index;
                    }
                }
                _ => return,
            }
            clicks >= 2
        };
        if confirm {
            self.forward("enter");
        }
        self.sync_view();
    }

    fn poll(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(200) {
            return;
        }
        self.last_poll = Instant::now();
        if let Some(app) = self.app.as_mut() {
            app.tick();
        }
    }

    fn begin_filter(&mut self) {
        let len = self.filter_text().chars().count();
        if let Some(app) = self.app.as_mut() {
            app.state.filter_snapshot = app.state.filter.clone();
            app.state.filter_editing = true;
        }
        self.filter_on = true;
        self.caret = len;
    }

    fn filter_key(&mut self, key: &str) {
        match key {
            "escape" => {
                if let Some(app) = self.app.as_mut() {
                    app.state.filter = app.state.filter_snapshot.clone();
                    app.state.filter_editing = false;
                    dd_dotstore::tree::flatten_visible(&mut app.state);
                }
                self.filter_on = false;
                self.caret = self.filter_text().chars().count();
            }
            "enter" => {
                if let Some(app) = self.app.as_mut() {
                    app.state.filter_editing = false;
                    dd_dotstore::tree::flatten_visible(&mut app.state);
                }
                self.filter_on = false;
            }
            "backspace" => self.filter_backspace(),
            "delete" => self.filter_delete(),
            "left" => self.caret = self.caret.saturating_sub(1),
            "right" => {
                let len = self.filter_text().chars().count();
                self.caret = (self.caret + 1).min(len);
            }
            "home" => self.caret = 0,
            "end" => self.caret = self.filter_text().chars().count(),
            other => {
                if let Some(ch) = typed_char(other) {
                    self.filter_insert(&ch.to_string());
                }
            }
        }
    }

    fn filter_insert(&mut self, text: &str) {
        let Some(app) = self.app.as_mut() else {
            return;
        };
        let byte = char_byte(&app.state.filter, self.caret);
        app.state.filter.insert_str(byte, text);
        self.caret += text.chars().count();
        dd_dotstore::tree::flatten_visible(&mut app.state);
    }

    fn filter_backspace(&mut self) {
        if self.caret == 0 {
            return;
        }
        let Some(app) = self.app.as_mut() else {
            return;
        };
        self.caret -= 1;
        let byte = char_byte(&app.state.filter, self.caret);
        app.state.filter.remove(byte);
        dd_dotstore::tree::flatten_visible(&mut app.state);
    }

    fn filter_delete(&mut self) {
        let Some(app) = self.app.as_mut() else {
            return;
        };
        let byte = char_byte(&app.state.filter, self.caret);
        if byte >= app.state.filter.len() {
            return;
        }
        app.state.filter.remove(byte);
        dd_dotstore::tree::flatten_visible(&mut app.state);
    }

    fn clear_filter(&mut self) {
        let Some(app) = self.app.as_mut() else {
            return;
        };
        if app.state.filter.is_empty() {
            return;
        }
        app.state.filter.clear();
        app.state.filter_editing = false;
        self.filter_on = false;
        self.caret = 0;
        dd_dotstore::tree::flatten_visible(&mut app.state);
    }

    fn choose_theme(&mut self, index: usize) {
        if self
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.editor.is_some())
        {
            return;
        }
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.page = Page::Themes;
            sheet.picker.select(index);
        }
        let preview = self
            .sheet
            .as_mut()
            .map(|sheet| sheet.picker.preview_selected());
        if let Some(preview) = preview {
            self.finish_picker(preview);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if is_paste(event) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.paste(&text);
            }
        } else {
            self.handle_key(&chord(event));
        }
        if self.quit && self.quit_runtime {
            cx.quit();
        }
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus.is_focused(window) {
            self.focus.focus(window, cx);
        }
        cx.set_reduce_motion(self.settings.reduced_motion);
        if self.quit && self.quit_runtime {
            cx.quit();
        }
        self.poll();
        let paint = paint_colors(self.app.as_ref());
        let footer = self.footer_line().to_string();
        let filter = self.filter_field_label();
        let quote = self.header_quote();
        let focus = self
            .focus_dump()
            .trim_start_matches("focus:")
            .trim()
            .to_string();
        let selected = self
            .app
            .as_ref()
            .and_then(|app| app.state.list_state.selected());
        let row_h = self.settings.density.row_h();
        let source_rows = self.source_paints();
        let dest_rows = self.destination_lines();
        let sheet = self.sheet_view(cx, &paint, row_h);
        let modal = self.modal_layer(cx, &paint, row_h);
        let toast = self.toast_layer(&paint, row_h);

        let mut tree = div()
            .id("tree-scroll")
            .track_scroll(&self.tree_scroll)
            .absolute()
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, row) in source_rows.into_iter().enumerate() {
            let on = selected == Some(index);
            tree = tree.child(
                div()
                    .id(("tree-row", index))
                    .h(px(row_h))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .when(on, |row| row.bg(paint.selected).text_color(paint.accent))
                    .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                        shell.click_source(index, event.modifiers().shift, event.click_count());
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id(("tree-check", index))
                            .cursor_pointer()
                            .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                                shell.click_checkbox(index);
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .child(row.check),
                    )
                    .child(
                        div()
                            .id(("tree-twist", index))
                            .cursor_pointer()
                            .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                                shell.click_twist(index);
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .child(if row.folder {
                                if row.expanded { "▾" } else { "▸" }
                            } else {
                                " "
                            }),
                    )
                    .child(row.line),
            );
        }

        let mut dest = div()
            .id("dest-scroll")
            .track_scroll(&self.dest_scroll)
            .absolute()
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for (index, line) in dest_rows.iter().cloned().enumerate() {
            dest = dest.child(
                div()
                    .id(("dest-row", index))
                    .h(px(row_h))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                        shell.click_destination(index);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(line),
            );
        }
        let dest_title = format!("Destinations ({})", dest_rows.len());

        div()
            .id("shell")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|shell, event: &KeyDownEvent, _, cx| {
                shell.on_key_down(event, cx);
                cx.notify();
            }))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(paint.base)
            .text_color(paint.text)
            .text_size(px(13.0))
            .child(band(&format!("dd_dotstore   {quote}"), &paint, true, row_h))
            .child(band(&focus, &paint, true, row_h))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .bg(paint.panel)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(paint.border)
                            .child(filter_row(cx, &filter, self.filter_on, &paint, row_h))
                            .children(self.load_error.as_ref().map(|err| {
                                div()
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .text_color(paint.text)
                                    .child(err.clone())
                            }))
                            .child(div().flex_1().min_h(px(0.0)).relative().child(tree)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .child(band(&dest_title, &paint, true, row_h))
                            .child(div().flex_1().min_h(px(0.0)).relative().child(dest)),
                    ),
            )
            .child(band(&footer, &paint, false, row_h))
            .children(sheet)
            .children(modal)
            .children(toast)
    }
}

impl Shell {
    fn filter_field_label(&self) -> String {
        let text = self.filter_text();
        if text.is_empty() && !self.filter_on {
            return "Filter  /".to_string();
        }
        let mut shown = String::new();
        for (index, ch) in text.chars().enumerate() {
            if self.filter_on && index == self.caret {
                shown.push('|');
            }
            shown.push(ch);
        }
        if self.filter_on && self.caret >= text.chars().count() {
            shown.push('|');
        }
        format!("Filter  {shown}")
    }

    fn sheet_view(
        &self,
        cx: &mut Context<Self>,
        paint: &Paint,
        row_h: f32,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let sheet = self.sheet.as_ref()?;
        let page = self.page_label(sheet);
        let hint = self.sheet_hint(sheet);
        let mut nav = div()
            .w(px(200.0))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_r_1()
            .border_color(paint.border)
            .child(
                div()
                    .id("settings-search")
                    .h(px(36.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_color(if sheet.query_on {
                        paint.text
                    } else {
                        paint.secondary
                    })
                    .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
                        if let Some(sheet) = shell.sheet.as_mut() {
                            sheet.query_on = true;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(if sheet.query.is_empty() {
                        "Search".to_string()
                    } else {
                        sheet.query.clone()
                    }),
            );
        for (index, label) in NAV.iter().enumerate() {
            let on = sheet.query.trim().is_empty() && sheet.page as usize == index;
            let page = Page::from_u8(index as u8);
            nav = nav.child(
                div()
                    .id(("settings-nav", index))
                    .h(px(32.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .when(on, |row| row.bg(paint.selected).text_color(paint.accent))
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                        if let Some(sheet) = shell.sheet.as_mut() {
                            sheet.page = page;
                            sheet.row = 0;
                            sheet.query.clear();
                            sheet.query_on = false;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(format!("{} {label}", if on { ">" } else { " " })),
            );
        }

        let mut scroll = div()
            .id("settings-scroll")
            .overflow_y_scroll()
            .track_scroll(&self.settings_scroll)
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .p(px(8.0));
        scroll = self.fill_scroll(scroll, sheet, paint, cx);

        let panel = div()
            .id("settings-panel")
            .occlude()
            .absolute()
            .top(px(row_h * 2.0))
            .right(px(0.0))
            .bottom(px(row_h))
            .w(relative(0.7))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(paint.panel)
            .text_color(paint.text)
            .border_l_1()
            .border_color(paint.border)
            .on_click(cx.listener(|_shell, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .h(px(48.0))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .border_b_1()
                    .border_color(paint.border)
                    .child("Settings")
                    .child(div().flex_1())
                    .child(div().text_color(paint.secondary).child("F1"))
                    .child(
                        div()
                            .id("settings-close")
                            .cursor_pointer()
                            .child("Close")
                            .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
                                shell.close_sheet(true);
                                cx.stop_propagation();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .overflow_hidden()
                    .child(nav)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h(px(CHROME))
                                    .px(px(8.0))
                                    .flex()
                                    .items_center()
                                    .font_weight(FontWeight::BOLD)
                                    .bg(paint.raised)
                                    .border_b_1()
                                    .border_color(paint.border)
                                    .child(page),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .text_color(paint.secondary)
                                    .child(hint),
                            )
                            .child(scroll),
                    ),
            );
        Some(panel)
    }

    fn fill_scroll(
        &self,
        mut scroll: gpui::Stateful<gpui::Div>,
        sheet: &Sheet,
        paint: &Paint,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if !sheet.query.trim().is_empty() {
            let hits = self.result_labels();
            if hits.is_empty() {
                return scroll.child(line("No matching settings", paint.secondary));
            }
            for (index, label) in hits.into_iter().enumerate() {
                let on = index == sheet.row;
                scroll = scroll.child(
                    div()
                        .id(("settings-result", index))
                        .h(px(CHROME))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .when(on, |row| row.bg(paint.selected))
                        .child(label),
                );
            }
            return scroll;
        }
        match sheet.page {
            Page::Themes => self.theme_scroll(scroll, sheet, paint, cx),
            Page::Window => self.window_scroll(scroll, sheet, paint, cx),
            Page::About => scroll.child(line(&self.about_text(), paint.text)),
            Page::Help => {
                let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
                for row in commands::shortcut_sheet() {
                    let text = commands::shortcut_line(row);
                    if groups
                        .last()
                        .is_some_and(|(section, _)| *section == row.section)
                    {
                        groups.last_mut().unwrap().1.push(text);
                    } else {
                        groups.push((row.section, vec![text]));
                    }
                }
                scroll
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .children(groups.into_iter().map(|(section, rows)| {
                        outlined(
                            section,
                            paint,
                            div()
                                .flex()
                                .flex_col()
                                .children(rows.into_iter().map(|row| line(&row, paint.text))),
                        )
                    }))
            }
        }
    }

    fn theme_scroll(
        &self,
        mut scroll: gpui::Stateful<gpui::Div>,
        sheet: &Sheet,
        paint: &Paint,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if let Some(editor) = &sheet.editor {
            for (index, field) in editor.fields.iter().enumerate() {
                let hex = editor
                    .palette
                    .get(field.key)
                    .map(|color| color.to_hex())
                    .unwrap_or_else(|| "#000000".to_string());
                let on = index == editor.selected;
                let mark = if on { ">" } else { " " };
                scroll = scroll.child(
                    div()
                        .id(("theme-color", index))
                        .h(px(CHROME))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .when(on, |row| row.bg(paint.selected))
                        .child(format!("{mark} {} {hex}", field.key)),
                );
            }
            scroll = scroll.child(line(&format!("Hex: {}", editor.hex_draft), paint.text));
            return scroll;
        }
        for (index, row) in sheet.picker.rows.iter().enumerate() {
            let on = index == sheet.picker.selected;
            let mark = if on { ">" } else { " " };
            let label = format!("{mark} {}", theme_row_label(row));
            scroll = scroll.child(
                div()
                    .id(("theme-row", index))
                    .h(px(CHROME))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .when(on, |row| row.bg(paint.selected).text_color(paint.accent))
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                        shell.choose_theme(index);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        if let Some(app) = &self.app {
            scroll = scroll
                .child(line(
                    &format!("Source: {}", app.state.theme.source.label()),
                    paint.secondary,
                ))
                .child(line(
                    &format!("Status: {}", app.state.theme_status.message),
                    paint.secondary,
                ));
        }
        let target = sheet.picker.save_target.label();
        scroll
            .child(line(&format!("Save to {target}"), paint.text))
            .child(line("Edit colors", paint.text))
            .child(line("Save", paint.text))
    }

    fn window_scroll(
        &self,
        scroll: gpui::Stateful<gpui::Div>,
        sheet: &Sheet,
        paint: &Paint,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let density = self.settings.density.label();
        let vim = if self.settings.vim_keys { "on" } else { "off" };
        let motion = if self.settings.reduced_motion {
            "on"
        } else {
            "off"
        };
        let rows = [
            (
                0usize,
                format!("Density {density}"),
                "Comfortable spacing, or Compact.",
            ),
            (
                1,
                format!("Vim keys {vim}"),
                "j k g G move lists when this is on.",
            ),
            (
                2,
                format!("Reduced motion {motion}"),
                "Theme changes paint immediately.",
            ),
        ];
        scroll
            .flex()
            .flex_col()
            .gap(px(12.0))
            .children(rows.map(|(index, title, detail)| {
                let on = sheet.row == index;
                outlined(
                    "Window",
                    paint,
                    div()
                        .id(("window-row", index))
                        .cursor_pointer()
                        .when(on, |row| row.bg(paint.selected))
                        .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| {
                            if let Some(sheet) = shell.sheet.as_mut() {
                                sheet.row = index;
                            }
                            shell.activate_window();
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .child(line(&title, paint.text))
                        .child(line(detail, paint.secondary)),
                )
            }))
    }

    fn sheet_hint(&self, sheet: &Sheet) -> String {
        if sheet.query_on {
            return "type to filter    up/down move    Enter    Esc clear".to_string();
        }
        if sheet.page == Page::Themes && sheet.editor.is_some() {
            let target = sheet
                .editor
                .as_ref()
                .map(|editor| editor.save_target.label())
                .unwrap_or("global");
            return format!("j/k move    Tab section    y save {target}    t target    Esc back");
        }
        if sheet.page == Page::Themes {
            return "j/k move    Enter    / search    Tab section    y save    t target    Esc revert"
                .to_string();
        }
        if sheet.page == Page::Help || sheet.page == Page::About {
            return "Tab section    Esc close".to_string();
        }
        "j/k move    Enter    / search    Tab section    Esc close".to_string()
    }

    fn modal_layer(
        &self,
        cx: &mut Context<Self>,
        paint: &Paint,
        row_h: f32,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let view = self.modal_dump()?;
        let mut body = div()
            .id("modal-body")
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0));
        for (index, row) in view.rows.iter().enumerate() {
            let mark = if row.selected { ">" } else { " " };
            let label = format!("{mark} {}", row.label);
            let clickable = row.index;
            body = body.child(
                div()
                    .id(("modal-row", index))
                    .h(px(row_h))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .when(row.selected, |row| row.bg(paint.selected))
                    .when(clickable.is_some(), |row| {
                        row.cursor_pointer().on_click(cx.listener(
                            move |shell, event: &ClickEvent, _, cx| {
                                if let Some(index) = clickable {
                                    shell.click_modal_row(index, event.click_count());
                                }
                                cx.stop_propagation();
                                cx.notify();
                            },
                        ))
                    })
                    .child(label),
            );
        }
        let kind = view.kind;
        Some(
            div()
                .id("modal-scrim")
                .absolute()
                .top(px(row_h * 2.0))
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(row_h))
                .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
                    shell.forward("escape");
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(
                    div()
                        .id("modal")
                        .absolute()
                        .top(px(24.0))
                        .left(relative(0.15))
                        .w(relative(0.7))
                        .max_h(relative(0.85))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .bg(paint.raised)
                        .border_1()
                        .border_color(paint.border)
                        .occlude()
                        .on_click(cx.listener(|_shell, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                        }))
                        .child(
                            div()
                                .h(px(row_h))
                                .px(px(8.0))
                                .flex()
                                .items_center()
                                .font_weight(FontWeight::BOLD)
                                .border_b_1()
                                .border_color(paint.border)
                                .child(view.title.clone()),
                        )
                        .child(body)
                        .child(
                            div()
                                .h(px(row_h))
                                .px(px(8.0))
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .border_t_1()
                                .border_color(paint.border)
                                .text_color(paint.secondary)
                                .child(view.hint.clone())
                                .when(matches!(kind, "plan" | "overwrite" | "adopt"), |row| {
                                    row.child(
                                        div()
                                            .id("modal-apply")
                                            .cursor_pointer()
                                            .text_color(paint.text)
                                            .on_click(cx.listener(
                                                |shell, _: &ClickEvent, _, cx| {
                                                    shell.forward("y");
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                },
                                            ))
                                            .child("Apply"),
                                    )
                                })
                                .child(
                                    div()
                                        .id("modal-cancel")
                                        .cursor_pointer()
                                        .text_color(paint.text)
                                        .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
                                            shell.forward("escape");
                                            cx.stop_propagation();
                                            cx.notify();
                                        }))
                                        .child("Cancel"),
                                ),
                        ),
                ),
        )
    }

    fn toast_layer(&self, paint: &Paint, row_h: f32) -> Option<gpui::Div> {
        let line = self.toast_dump()?;
        Some(
            div()
                .absolute()
                .bottom(px(row_h + 8.0))
                .right(px(12.0))
                .max_w(relative(0.6))
                .px(px(8.0))
                .py(px(4.0))
                .bg(paint.raised)
                .border_1()
                .border_color(paint.border)
                .text_color(paint.text)
                .child(line),
        )
    }
}

struct SourcePaint {
    line: String,
    check: &'static str,
    folder: bool,
    expanded: bool,
}

fn theme_row_label(row: &PickerRow) -> String {
    match row {
        PickerRow::Follow { name } => format!("Follow desktop {name}"),
        PickerRow::Theme(theme) => format!("{} {}", theme.name, theme.mode.label()),
        PickerRow::Custom => "Custom built-in".to_string(),
    }
}

fn section_dir(key: &str, hex: bool) -> Option<i32> {
    match key {
        "tab" => Some(1),
        "shift-tab" | "backtab" => Some(-1),
        "]" if !hex => Some(1),
        "[" if !hex => Some(-1),
        _ => None,
    }
}

#[cfg(test)]
fn density_token(density: Density) -> &'static str {
    match density {
        Density::Comfortable => "comfortable",
        Density::Compact => "compact",
    }
}

fn typed_char(key: &str) -> Option<char> {
    if key == "space" {
        return Some(' ');
    }
    if let Some(rest) = key.strip_prefix("shift-") {
        let mut chars = rest.chars();
        let ch = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        return Some(match ch {
            'a'..='z' => ch.to_ascii_uppercase(),
            _ => ch,
        });
    }
    let mut chars = key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_control() {
        return None;
    }
    Some(ch)
}

fn char_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn chord(event: &KeyDownEvent) -> String {
    let mut parts = Vec::new();
    let mods = event.keystroke.modifiers;
    if mods.control || mods.platform {
        parts.push("ctrl".to_string());
    }
    if mods.shift {
        parts.push("shift".to_string());
    }
    if mods.alt {
        parts.push("alt".to_string());
    }
    parts.push(event.keystroke.key.to_ascii_lowercase());
    parts.join("-")
}

fn is_paste(event: &KeyDownEvent) -> bool {
    let mods = event.keystroke.modifiers;
    (mods.control || mods.platform)
        && event.keystroke.key.eq_ignore_ascii_case("v")
        && !mods.shift
        && !mods.alt
}

struct Paint {
    base: Hsla,
    panel: Hsla,
    raised: Hsla,
    text: Hsla,
    secondary: Hsla,
    selected: Hsla,
    border: Hsla,
    accent: Hsla,
}

fn paint_colors(app: Option<&App>) -> Paint {
    let theme = app.map(|app| &app.state.theme);
    let pack = |color| {
        let (r, g, b) = dd_dotstore::theme::color_rgb(color);
        gpui::Rgba {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        }
        .into()
    };
    if let Some(theme) = theme {
        let colors = &theme.colors;
        Paint {
            base: pack(colors.base_background),
            panel: pack(colors.body_background),
            raised: pack(colors.modal_background),
            text: pack(colors.text_primary),
            secondary: pack(colors.text_secondary),
            selected: pack(colors.selected_background),
            border: pack(colors.border_default),
            accent: pack(colors.border_active),
        }
    } else {
        let theme = dd_dotstore::theme::Theme::default();
        let colors = &theme.colors;
        Paint {
            base: pack(colors.base_background),
            panel: pack(colors.body_background),
            raised: pack(colors.modal_background),
            text: pack(colors.text_primary),
            secondary: pack(colors.text_secondary),
            selected: pack(colors.selected_background),
            border: pack(colors.border_default),
            accent: pack(colors.border_active),
        }
    }
}

fn band(text: &str, paint: &Paint, top: bool, height: f32) -> gpui::Div {
    div()
        .h(px(height))
        .px(px(8.0))
        .flex()
        .items_center()
        .flex_shrink_0()
        .bg(paint.raised)
        .text_color(paint.text)
        .font_weight(FontWeight::BOLD)
        .when(top, |row| row.border_b_1().border_color(paint.border))
        .when(!top, |row| row.border_t_1().border_color(paint.border))
        .child(text.to_string())
}

fn filter_row(
    cx: &mut Context<Shell>,
    label: &str,
    on: bool,
    paint: &Paint,
    height: f32,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id("tree-filter")
        .h(px(height))
        .px(px(8.0))
        .flex()
        .items_center()
        .flex_shrink_0()
        .border_b_1()
        .border_color(if on { paint.accent } else { paint.border })
        .text_color(if on { paint.text } else { paint.secondary })
        .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| {
            shell.begin_filter();
            cx.stop_propagation();
            cx.notify();
        }))
        .child(label.to_string())
}

fn line(text: &str, color: Hsla) -> gpui::Div {
    div().w_full().text_color(color).child(text.to_string())
}

fn outlined(title: &str, paint: &Paint, body: impl IntoElement) -> gpui::Div {
    div()
        .border_1()
        .border_color(paint.border)
        .child(
            div()
                .h(px(CHROME))
                .px(px(8.0))
                .flex()
                .items_center()
                .font_weight(FontWeight::BOLD)
                .bg(paint.raised)
                .border_b_1()
                .border_color(paint.border)
                .child(title.to_string()),
        )
        .child(div().p(px(8.0)).child(body))
}
