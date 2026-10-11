mod destinations;
mod focus;
mod modals;
mod source;
pub mod toast;

use crate::domain::AppState;
use destinations::draw_status_panel;
use modals::draw_modal;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
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
    f.render_widget(Block::default().style(state.theme.app_shell), f.area());

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // quote plus the focus line
            Constraint::Min(0),
            Constraint::Length(1), // footer is now just the key hints line (decluttered)
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

    if state.modal.is_some() {
        draw_modal(f, state, f.area());
    }

    if state.toast.is_some() {
        draw_toast(f, state, f.area());
    }
}

fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let lines = vec![
        Line::from(state.header_copy.as_str()),
        focus::focus_header_line(state, inner_width),
    ];
    let header = Paragraph::new(lines).block(
        Block::default()
            .title("dd_dotstore")
            .borders(Borders::ALL)
            .border_style(state.theme.active_border)
            .style(state.theme.app_shell),
    );
    f.render_widget(header, area);
}

fn draw_status_bar(f: &mut Frame, state: &AppState, area: Rect) {
    // Width-adaptive key hints (footer reduced to 1 line; theme status moved out of persistent footer
    // to reduce clutter — full details still in F2 Theme / F3 Credits and at startup).
    let keys = if area.width < 75 {
        "F1:Help  q:Quit  j/k:Nav  Spc:Sel  s:Apply  x:Rem  /:Filter"
    } else if area.width < 110 {
        "F1: Help   /: Search   Space: Select   m/M: Link/Copy   s: Apply   x: Remove   q: Quit"
    } else {
        "F1: Help   F2: Theme   t: Group   A: Adopt   D: Doctor   s: Apply   q: Quit   (mouse: click/scroll/drag)"
    };

    let bar = Paragraph::new(Line::from(keys))
        .block(Block::default())
        .style(state.theme.app_shell);
    f.render_widget(bar, area);
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
    use super::draw;
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

        let focus = row_text(terminal.backend().buffer(), 2);
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
            3,
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
        let suggested = row_text(terminal.backend().buffer(), 2);
        assert!(
            suggested.contains("suggested (not saved)"),
            "an unsaved row should not look mapped: {suggested:?}"
        );
        assert!(
            !suggested.contains("  linked"),
            "unsaved status must replace linked: {suggested:?}"
        );
    }
}
