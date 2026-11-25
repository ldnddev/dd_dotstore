mod destinations;
mod focus;
mod modals;
pub mod settings;
mod source;
pub mod toast;
mod toast_chrome;

use crate::domain::AppState;
use destinations::draw_status_panel;
use modals::draw_modal;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use settings::draw_settings;
use source::draw_source_panel;
use toast::draw_toast;

pub use modals::{overwrite_conflict_row, overwrite_warning_counts};

pub fn draw(f: &mut Frame, state: &mut AppState) {
    state.pointer.last_frame_area = f.area();
    if state.toast.is_none() {
        state.pointer.toast_area = None;
    }
    if state.modal.is_none() {
        state.pointer.current_modal_area = None;
    }
    state.pointer.settings_nav_area = Rect::default();
    f.render_widget(Block::default().style(state.theme.app_shell), f.area());

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, state, outer[0]);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[1]);

    draw_source_panel(f, state, chunks[0]);
    draw_status_panel(f, state, chunks[1]);
    draw_status_bar(f, state, outer[2]);

    if state.show_settings {
        draw_settings(f, state, outer[1]);
    }

    if state.modal.is_some() {
        draw_modal(f, state, f.area());
    }

    if state.toast.is_some() {
        draw_toast(f, state, f.area());
    }
}

fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let name = Line::from(Span::styled(
        " dd_dotstore ",
        Style::default()
            .fg(state.theme.colors.text_primary)
            .add_modifier(Modifier::BOLD),
    ))
    .left_aligned();
    let mut block = Block::default()
        .title(name)
        .borders(Borders::ALL)
        .border_style(state.theme.active_border)
        .style(state.theme.app_shell);
    let quote_budget = area.width.saturating_sub(16).saturating_sub(2) as usize;
    if quote_budget >= 4 && !state.header_copy.is_empty() {
        let quote = ellipsize_end(&state.header_copy, quote_budget);
        block = block.title(
            Line::from(Span::styled(
                format!(" {quote} "),
                Style::default().fg(state.theme.colors.text_secondary),
            ))
            .right_aligned(),
        );
    }
    let header = Paragraph::new(focus::focus_header_line(state, inner_width)).block(block);
    f.render_widget(header, area);
}

fn draw_status_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let bar = Paragraph::new(Line::from(footer_hint(state, area.width)))
        .block(Block::default())
        .style(state.theme.app_shell);
    f.render_widget(bar, area);
}

/// One footer row. Drops middle hints from the right when `width` is tight.
/// F1 and quit stay.
pub(crate) fn footer_hint(state: &AppState, width: u16) -> String {
    if state.show_settings || state.modal.is_some() {
        return if width < 80 {
            "F1:Settings  Esc:Close  ^Q:Quit".to_string()
        } else {
            "F1: Settings  Esc: Close  Ctrl+Q: Quit".to_string()
        };
    }
    let (f1, quit) = if width < 80 {
        ("F1:Settings", "^Q:Quit")
    } else {
        ("F1: Settings", "Ctrl+Q: Quit")
    };
    let middle: &[&str] = if width < 80 {
        &["j/k:Nav", "Spc:Sel", "s:Apply", "/:Filter"]
    } else {
        &["j/k: Move", "Space: Select", "s: Apply", "/: Filter"]
    };
    fit_footer(width, &[f1], middle, &[quit])
}

fn fit_footer(width: u16, left: &[&str], middle: &[&str], right: &[&str]) -> String {
    let limit = width as usize;
    let mut mid: Vec<&str> = middle.to_vec();
    loop {
        let mut parts = Vec::with_capacity(left.len() + mid.len() + right.len());
        parts.extend_from_slice(left);
        parts.extend(mid.iter().copied());
        parts.extend_from_slice(right);
        let line = parts.join("  ");
        if line.chars().count() <= limit || mid.is_empty() {
            return line;
        }
        mid.pop();
    }
}

fn ellipsize_end(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::{draw, footer_hint};
    use crate::domain::{ActionMode, AppState, Node, NodeKind, SymlinkStatus};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use std::path::PathBuf;

    fn saved_file(name: &str) -> Node {
        Node {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind: NodeKind::File {
                dest: Some(PathBuf::from(format!("/tmp/dd-home/{name}"))),
            },
            selected: false,
            action_mode: ActionMode::Symlink,
            symlink_status: SymlinkStatus::Valid,
            group: None,
            has_configured_descendant: false,
        }
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf.cell((x, y)).map(|cell| cell.symbol()).unwrap_or(""))
            .collect()
    }

    fn right_pane_has_selection(buf: &Buffer, fg: ratatui::style::Color) -> bool {
        (4..11).any(|y| {
            (40..80).any(|x| {
                buf.cell((x, y))
                    .is_some_and(|cell| cell.fg == fg && cell.symbol() != " ")
            })
        })
    }

    #[test]
    fn focus_line_and_destination_follow_the_highlighted_row() {
        let mut state = AppState::default_with_root(PathBuf::from("/tmp/dd-dots"));
        state.header_copy = "Don't Fear the . (Dot) - Tame It.".into();
        state.tree = (0..8)
            .map(|i| saved_file(&format!("file{i}.txt")))
            .collect();
        state.nodes = state.tree.clone();
        state.list_state.select(Some(7));

        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, &mut state))
            .expect("draw");

        let focus = row_text(terminal.backend().buffer(), 1);
        assert!(
            focus.contains("file7.txt"),
            "focus line should name the highlighted source: {focus:?}"
        );
        assert!(
            focus.contains("/tmp/dd-home/file7.txt"),
            "focus line should show the saved destination: {focus:?}"
        );
        assert!(
            focus.contains("linked"),
            "a valid symlink should say linked: {focus:?}"
        );
        assert_eq!(
            state.status_list_state.offset(),
            2,
            "the matching destination starts below the fold and should scroll up"
        );
        let selected_fg = state.theme.colors.text_active_focus;
        assert!(
            right_pane_has_selection(terminal.backend().buffer(), selected_fg),
            "the matching destination row should use the selection color"
        );

        *state.status_list_state.offset_mut() = 0;
        terminal
            .draw(|frame| draw(frame, &mut state))
            .expect("draw");
        assert_eq!(
            state.status_list_state.offset(),
            0,
            "scrolling Destinations while the source cursor stays put should stick"
        );

        state.nodes[7].kind = NodeKind::File { dest: None };
        state.nodes[7].symlink_status = SymlinkStatus::None;
        state.tree[7].kind = NodeKind::File { dest: None };
        state.tree[7].symlink_status = SymlinkStatus::None;
        terminal
            .draw(|frame| draw(frame, &mut state))
            .expect("draw");
        let suggested = row_text(terminal.backend().buffer(), 1);
        assert!(
            suggested.contains("suggested (not saved)"),
            "an unsaved row should not look mapped: {suggested:?}"
        );
        assert!(
            !suggested.contains("  linked"),
            "unsaved status must replace linked: {suggested:?}"
        );
    }

    #[test]
    fn footer_starts_with_settings_and_ends_with_ctrl_q() {
        let state = AppState::default_with_root(PathBuf::from("/tmp/dd-dots"));
        let narrow = footer_hint(&state, 79);
        assert!(narrow.starts_with("F1:Settings"), "{narrow}");
        assert!(narrow.ends_with("^Q:Quit"), "{narrow}");
        assert!(narrow.contains("j/k:Nav"), "{narrow}");
        let wide = footer_hint(&state, 80);
        assert!(wide.starts_with("F1: Settings"), "{wide}");
        assert!(wide.ends_with("Ctrl+Q: Quit"), "{wide}");
        assert!(wide.contains("j/k: Move"), "{wide}");
        assert!(
            !wide.contains("q:Quit") && !wide.contains("q: Quit"),
            "{wide}"
        );
        assert!(
            !wide.contains("F1:Help") && !wide.contains("F1: Help"),
            "{wide}"
        );
        assert!(!wide.contains("F2"), "{wide}");
        let tight = footer_hint(&state, 20);
        assert_eq!(tight, "F1:Settings  ^Q:Quit");
    }

    #[test]
    fn footer_names_esc_while_the_sheet_or_a_modal_is_open() {
        let mut state = AppState::default_with_root(PathBuf::from("/tmp/dd-dots"));
        state.show_settings = true;
        assert_eq!(footer_hint(&state, 40), "F1:Settings  Esc:Close  ^Q:Quit");
        assert_eq!(
            footer_hint(&state, 80),
            "F1: Settings  Esc: Close  Ctrl+Q: Quit"
        );
        state.show_settings = false;
        state.modal = Some(crate::domain::Modal::IgnoreEditor {
            selected: 0,
            draft: String::new(),
        });
        assert_eq!(footer_hint(&state, 79), "F1:Settings  Esc:Close  ^Q:Quit");
    }

    #[test]
    fn settings_sheet_covers_only_the_right_half() {
        let mut state = AppState::default_with_root(PathBuf::from("/tmp/dd-dots"));
        state.header_copy = "Don't Fear the . (Dot) - Tame It.".into();
        state.tree = vec![saved_file("file0.txt")];
        state.nodes = state.tree.clone();
        state.list_state.select(Some(0));
        state.show_settings = true;

        let backend = TestBackend::new(80, 16);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw(frame, &mut state))
            .expect("draw");
        let buf = terminal.backend().buffer().clone();
        let top = row_text(&buf, 3);
        assert!(
            top.contains("Settings"),
            "sheet title sits on the body band: {top:?}"
        );
        let themes = row_text(&buf, 4);
        let about = row_text(&buf, 5);
        let help = row_text(&buf, 6);
        assert!(
            themes.contains("> Themes"),
            "selected nav row is > Themes: {themes:?}"
        );
        assert!(about.contains("About"), "{about:?}");
        assert!(help.contains("Help"), "{help:?}");
        let left: String = (0..40)
            .map(|x| buf.cell((x, 4)).map(|cell| cell.symbol()).unwrap_or(""))
            .collect();
        assert!(
            left.contains("file0.txt"),
            "the body under the left half stays visible: {left:?}"
        );
    }
}
