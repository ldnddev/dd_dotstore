use crate::domain::{ActionMode, AppState, Node, NodeKind};
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::path::PathBuf;

pub(crate) fn draw_status_panel(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.pointer.status_area = area;
    let mut lines: Vec<Line<'_>> = vec![];
    state.pointer.destination_paths.clear();

    fn collect_destinations<'a>(
        node: &Node,
        out: &mut Vec<Line<'a>>,
        paths: &mut Vec<PathBuf>,
        theme: &Theme,
    ) {
        match &node.kind {
            NodeKind::File { dest: Some(d) } | NodeKind::Folder { dest: Some(d), .. } => {
                let arrow = match node.action_mode {
                    ActionMode::Symlink => "->",
                    ActionMode::Copy => "=>",
                };
                let mode_style = match node.action_mode {
                    ActionMode::Symlink => theme.link,
                    ActionMode::Copy => theme.warning,
                };
                let kind_style = match node.kind {
                    NodeKind::Folder { .. } => theme.folder,
                    NodeKind::File { .. } => theme.file,
                };
                out.push(Line::from(vec![
                    Span::styled("• ", mode_style),
                    Span::styled(node.action_mode.label().to_string(), mode_style),
                    Span::raw(" "),
                    Span::styled(node.path.display().to_string(), kind_style),
                    Span::raw(format!(" {arrow} ")),
                    Span::styled(d.display().to_string(), mode_style),
                ]));
                paths.push(node.path.clone());
            }
            NodeKind::File { dest: None } | NodeKind::Folder { dest: None, .. } => {}
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            for c in children {
                collect_destinations(c, out, paths, theme);
            }
        }
    }

    for node in &state.tree {
        collect_destinations(
            node,
            &mut lines,
            &mut state.pointer.destination_paths,
            &state.theme,
        );
    }

    let dest_count = lines.len();
    let title = format!("Destinations ({})", dest_count);

    let items: Vec<ListItem<'_>> = lines.into_iter().map(ListItem::new).collect();

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(state.theme.border)
            .style(state.theme.body),
    );

    f.render_stateful_widget(list, area, &mut state.status_list_state);

    let total = dest_count;
    let view_rows = area.height.saturating_sub(2) as usize;
    if total > view_rows && view_rows > 0 {
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(state.status_list_state.offset())
            .viewport_content_length(view_rows);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .thumb_style(state.theme.scrollbar);
        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
