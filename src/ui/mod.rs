mod destinations;
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
            Constraint::Length(3),
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
    let header = Paragraph::new(state.header_copy.as_str()).block(
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
    // to reduce clutter — full details still in F2 Credits and at startup).
    let keys = if area.width < 75 {
        "F1:Help  q:Quit  j/k:Nav  Spc:Sel  s:Apply  x:Rem  /:Filter"
    } else if area.width < 110 {
        "F1: Help   /: Search   Space: Select   m/M: Link/Copy   s: Apply   x: Remove   q: Quit"
    } else {
        "F1: Help   C: Theme   t: Group   A: Adopt   D: Doctor   s: Apply   q: Quit   (mouse: click/scroll/drag)"
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
