//! F1 Settings sheet. Right half of the body. Themes, About, Help.

use crate::domain::AppState;
use crate::theme::{PickerRow, ThemeEditorRow, theme_editor_rows};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub const NAV: [&str; 3] = ["Themes", "About", "Help"];
pub const PAGE_THEMES: u8 = 0;
pub const PAGE_ABOUT: u8 = 1;
pub const PAGE_HELP: u8 = 2;
pub const PAGE_LAST: u8 = 2;

const HELP_TEXT: &str = "\
Keybindings\n\
\n\
F1           Settings (Themes, About, Help)\n\
F2           Settings (same toggle as F1)\n\
F3           Settings, About page\n\
Ctrl+Q       Quit\n\
Esc          Close settings / clear filter / clear selection (does not quit)\n\
j/k or ↑/↓   Navigate\n\
Space        Toggle selection (file/folder)\n\
Enter        Edit destination (file/folder)\n\
e            Edit destination (file/folder)\n\
h/l or ←/→   Collapse/expand folder\n\
s / p        Plan apply (highlight or checkboxes; Y applies)\n\
x            Plan remove (highlight or checkboxes; Y applies)\n\
m            Toggle LINK/COPY for highlighted item\n\
M            Set selected items to the next LINK/COPY mode\n\
t            Set group on highlight or checkboxes\n\
T            Select every item in the highlight's group\n\
A            Reverse-import HOME/XDG symlinks that point at this project\n\
D            Doctor: broken, planned, unknown, and orphan links\n\
u            Undo last action\n\
/            Focus source filter (does not clear; also matches group names)\n\
r            Reload tree\n\
I            Ignore editor (list / add / delete, persisted)\n\
i            Import picker\n\
E            Export picker\n\
\n\
Mouse\n\
Click row           Highlight\n\
Far-left click      Toggle [ ] select\n\
Click tree area     Toggle expand (folders)\n\
Shift+click         Range multi-select from current to clicked\n\
Double-click name   Edit destination\n\
Wheel over list     Scroll source (Shift=faster)\n\
Wheel over right    Scroll Destinations\n\
Drag right scrollbar Scroll the view\n\
Click right panel   Jump focus + auto-expand ancestors\n\
Click outside modal Close it\n\
Click inside a confirm/overwrite/plan dialog does nothing — use Y\n\
Browser: click moves, double-click picks, scrollbar drag works\n\
\n\
Tree: proper connectors (├ └ │) + counts + subtree badges (●) for density.\n\
○ planned dest (assigned, not on disk). Unassigned rows show dim → dest.";

pub fn draw_settings(frame: &mut Frame, state: &mut AppState, body: Rect) {
    if body.width == 0 || body.height == 0 {
        state.pointer.settings_nav_area = Rect::default();
        return;
    }
    let width = (body.width / 2).max(1).min(body.width);
    let area = Rect {
        x: body.x + body.width.saturating_sub(width),
        y: body.y,
        width,
        height: body.height,
    };
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(state.theme.active_border)
        .title_style(
            Style::default()
                .fg(state.theme.modal_header)
                .add_modifier(Modifier::BOLD),
        )
        .style(
            Style::default()
                .bg(state.theme.colors.modal_background)
                .fg(state.theme.colors.modal_text),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        state.pointer.settings_nav_area = Rect::default();
        return;
    }

    if state.settings_nav as usize >= NAV.len() {
        state.settings_nav = 0;
    }

    let nav_w = {
        let spare = inner.width.saturating_sub(8);
        if spare == 0 {
            1.min(inner.width)
        } else {
            14.min(spare).max(1)
        }
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(nav_w), Constraint::Min(1)])
        .split(inner);
    let nav = cols[0];
    let page = cols[1];
    state.pointer.settings_nav_area = nav;
    paint_nav(frame, state, nav);

    let section = NAV
        .get(state.settings_nav as usize)
        .copied()
        .unwrap_or("Themes");
    let title_h = u16::from(page.height > 0);
    let title_area = Rect {
        x: page.x,
        y: page.y,
        width: page.width,
        height: title_h,
    };
    let content = Rect {
        x: page.x,
        y: page.y.saturating_add(title_h),
        width: page.width,
        height: page.height.saturating_sub(title_h),
    };
    if title_h > 0 {
        let shown: String = section.chars().take(page.width as usize).collect();
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                shown,
                Style::default()
                    .fg(state.theme.modal_header)
                    .add_modifier(Modifier::BOLD),
            )))
            .style(
                Style::default()
                    .bg(state.theme.colors.modal_background)
                    .fg(state.theme.modal_header),
            ),
            title_area,
        );
    }

    match state.settings_nav {
        PAGE_ABOUT => {
            let text = about_text(state);
            let lines = text_lines(state, &text, content.width.max(1) as usize);
            paint_lines(frame, state, content, lines, None);
        }
        PAGE_HELP => {
            let lines = text_lines(state, HELP_TEXT, content.width.max(1) as usize);
            paint_lines(frame, state, content, lines, None);
        }
        _ => paint_themes(frame, state, content),
    }
}

pub fn about_copy(source_label: &str, status: &str) -> String {
    format!(
        "Credits\n\
\n\
- GNU Stow: dotfile workflow inspiration\n\
- Ratatui + Crossterm: TUI stack\n\
\n\
Theme source: {source_label}\n\
Theme status: {status}\n\
\n\
Themes are on this sheet. j/k preview, Enter apply, e colors, y/s save.\n\
Esc closes Settings."
    )
}

fn about_text(state: &AppState) -> String {
    about_copy(state.theme.source.label(), &state.theme_status.message)
}

fn paint_nav(frame: &mut Frame, state: &AppState, nav: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(state.theme.colors.border_default))
        .style(
            Style::default()
                .bg(state.theme.colors.body_background)
                .fg(state.theme.colors.modal_text),
        );
    let inner = block.inner(nav);
    frame.render_widget(block, nav);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let selected_style = Style::default()
        .fg(state.theme.colors.text_active_focus)
        .bg(state.theme.colors.selected_background);
    let idle = Style::default()
        .fg(state.theme.colors.modal_text)
        .bg(state.theme.colors.body_background);
    let width = inner.width as usize;
    let mut lines = Vec::new();
    for (index, label) in NAV.iter().enumerate() {
        let on = state.settings_nav == index as u8;
        let marker = if on { ">" } else { " " };
        let text: String = format!("{marker} {label}").chars().take(width).collect();
        let style = if on { selected_style } else { idle };
        lines.push(Line::from(Span::styled(text, style)));
    }
    frame.render_widget(
        Paragraph::new(lines).style(
            Style::default()
                .bg(state.theme.colors.body_background)
                .fg(state.theme.colors.modal_text),
        ),
        inner,
    );
}

fn paint_themes(frame: &mut Frame, state: &mut AppState, content: Rect) {
    let editor = state.theme_editor.clone();
    let picker = state.theme_picker.clone();
    let box_w = (content.width as usize).max(2);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut selected_line = None;
    let rule = Style::default().fg(state.theme.colors.border_default);
    let header = Style::default()
        .fg(state.theme.modal_header)
        .add_modifier(Modifier::BOLD);

    let hint = if let Some(editor) = &editor {
        let target = editor.save_target.label();
        if picker.is_some() {
            format!("Tab section   j/k move   t target   y save {target}   R reset   Esc back")
        } else {
            format!("Tab section   j/k move   t target   y save {target}   R reset   Esc revert")
        }
    } else {
        "Tab section   j/k move   Enter apply   e colors   y save   t target   Esc revert"
            .to_string()
    };
    lines.push(Line::from(Span::styled(
        hint,
        Style::default().fg(state.theme.colors.text_secondary),
    )));

    if let Some(editor) = &editor {
        if editor.editing_hex {
            let cursor = editor.hex_draft.chars().count();
            lines.push(caret_line("Hex: ", &editor.hex_draft, cursor));
        } else {
            lines.push(Line::from(format!(
                "Hex: {}   Enter edit",
                editor.hex_draft
            )));
        }
        let selected_style = Style::default()
            .fg(state.theme.colors.text_active_focus)
            .bg(state.theme.colors.selected_background);
        let idle = Style::default().fg(state.theme.colors.modal_text);
        let mut open = false;
        for row in theme_editor_rows(&editor.fields) {
            match row {
                ThemeEditorRow::Header(name) => {
                    if open {
                        lines.push(settings_bottom(box_w, rule));
                    }
                    lines.push(Line::from(""));
                    lines.push(settings_top(name, box_w, rule, header));
                    open = true;
                }
                ThemeEditorRow::Color(idx) => {
                    if idx == editor.selected {
                        selected_line = Some(lines.len());
                    }
                    let field = editor.fields[idx];
                    let hex = editor
                        .palette
                        .get(field.key)
                        .map(|color| color.to_hex())
                        .unwrap_or_else(|| "#000000".into());
                    let cursor = if idx == editor.selected { ">" } else { " " };
                    let style = if idx == editor.selected {
                        selected_style
                    } else {
                        idle
                    };
                    lines.push(settings_row(
                        &format!("{cursor} {:<22} {hex}", field.key),
                        box_w,
                        rule,
                        style,
                    ));
                }
            }
        }
        if open {
            lines.push(settings_bottom(box_w, rule));
        }
    } else if let Some(picker) = &picker {
        lines.push(Line::from(""));
        lines.push(settings_top("Themes", box_w, rule, header));
        let selected_style = Style::default()
            .fg(state.theme.colors.text_active_focus)
            .bg(state.theme.colors.selected_background);
        let idle = Style::default().fg(state.theme.colors.modal_text);
        for (idx, row) in picker.rows.iter().enumerate() {
            if idx == picker.selected {
                selected_line = Some(lines.len());
            }
            let (label, detail) = match row {
                PickerRow::Follow { name } => ("Follow desktop".to_string(), name.clone()),
                PickerRow::Theme(theme) => (theme.name.clone(), theme.mode.label().to_string()),
                PickerRow::Custom => ("Custom".to_string(), "built-in".to_string()),
            };
            let cursor = if idx == picker.selected { ">" } else { " " };
            let style = if idx == picker.selected {
                selected_style
            } else {
                idle
            };
            lines.push(settings_row(
                &format!("{cursor} {label:<22} {detail}"),
                box_w,
                rule,
                style,
            ));
        }
        lines.push(settings_bottom(box_w, rule));
    }

    paint_lines(frame, state, content, lines, selected_line);
}

fn text_lines(state: &AppState, text: &str, width: usize) -> Vec<Line<'static>> {
    let header = Style::default()
        .fg(state.theme.modal_header)
        .add_modifier(Modifier::BOLD);
    let body = Style::default().fg(state.theme.colors.modal_text);
    let mut lines = Vec::new();
    for raw in text.split('\n') {
        let style = if is_section_header(raw) { header } else { body };
        if raw.is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        for piece in wrap_line(raw, width.max(1)) {
            lines.push(Line::from(Span::styled(piece, style)));
        }
    }
    lines
}

fn is_section_header(line: &str) -> bool {
    matches!(line, "Keybindings" | "Mouse" | "Credits") || line.starts_with("Tree:")
}

fn wrap_line(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if text.chars().count() <= width {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if word.chars().count() > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut chunk = String::new();
            for ch in word.chars() {
                if chunk.chars().count() == width {
                    lines.push(std::mem::take(&mut chunk));
                }
                chunk.push(ch);
            }
            current = chunk;
            continue;
        }
        let extra = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if extra > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        } else if current.is_empty() {
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if current.is_empty() {
        lines.push(String::new());
    } else {
        lines.push(current);
    }
    lines
}

fn settings_top(title: &str, width: usize, edge: Style, title_style: Style) -> Line<'static> {
    let width = width.max(2);
    let inner = width - 2;
    let mut label = format!(" {title} ");
    if label.chars().count() > inner {
        label = label.chars().take(inner).collect();
    }
    let pad = inner - label.chars().count();
    Line::from(vec![
        Span::styled("┌", edge),
        Span::styled(label, title_style),
        Span::styled(format!("{}┐", "─".repeat(pad)), edge),
    ])
}

fn settings_bottom(width: usize, edge: Style) -> Line<'static> {
    let width = width.max(2);
    Line::from(Span::styled(format!("└{}┘", "─".repeat(width - 2)), edge))
}

fn settings_row(body: &str, width: usize, edge: Style, fill: Style) -> Line<'static> {
    let width = width.max(2);
    let inner = width - 2;
    let mut text: String = body.chars().take(inner).collect();
    let pad = inner - text.chars().count();
    text.push_str(&" ".repeat(pad));
    Line::from(vec![
        Span::styled("│", edge),
        Span::styled(text, fill),
        Span::styled("│", edge),
    ])
}

fn caret_line(prefix: &str, text: &str, cursor: usize) -> Line<'static> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    let cell = chars
        .get(cursor)
        .map(|ch| ch.to_string())
        .unwrap_or_else(|| " ".to_string());
    let after: String = chars.get(cursor + 1..).unwrap_or(&[]).iter().collect();
    Line::from(vec![
        Span::raw(prefix.to_string()),
        Span::raw(before),
        Span::styled(cell, Style::default().add_modifier(Modifier::REVERSED)),
        Span::raw(after),
    ])
}

fn paint_lines(
    frame: &mut Frame,
    state: &mut AppState,
    area: Rect,
    lines: Vec<Line<'static>>,
    selected_line: Option<usize>,
) {
    if area.width == 0 || area.height == 0 {
        state.help_scroll_max = 0;
        state.help_scroll = 0;
        return;
    }
    let view_h = area.height as usize;
    let max_scroll = lines.len().saturating_sub(view_h) as u16;
    state.help_scroll_max = max_scroll;
    if state.settings_follow {
        if let Some(sel) = selected_line {
            let top = state.help_scroll as usize;
            if sel < top {
                state.help_scroll = sel as u16;
            } else if sel >= top + view_h {
                state.help_scroll = (sel + 1 - view_h) as u16;
            }
        }
        state.settings_follow = false;
    }
    if state.help_scroll > max_scroll {
        state.help_scroll = max_scroll;
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(
                Style::default()
                    .fg(state.theme.colors.modal_text)
                    .bg(state.theme.colors.modal_background),
            )
            .scroll((state.help_scroll, 0)),
        area,
    );
}
