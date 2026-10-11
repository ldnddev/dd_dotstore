use crate::domain::{ActionMode, AppState, Node, NodeKind};
use crate::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::path::{Path, PathBuf};

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
                let mut spans = vec![
                    Span::styled("• ", mode_style),
                    Span::styled(node.action_mode.label().to_string(), mode_style),
                    Span::raw(" "),
                    Span::styled(node.path.display().to_string(), kind_style),
                ];
                if let Some(group) = node.group.as_ref() {
                    spans.push(Span::styled(format!(" ·{group}"), theme.secondary));
                }
                spans.push(Span::raw(format!(" {arrow} ")));
                spans.push(Span::styled(d.display().to_string(), mode_style));
                out.push(Line::from(spans));
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

    // Scroll the matching saved row into view only when the source cursor
    // changes. A wheel over this pane keeps its offset until then.
    let view_rows = area.height.saturating_sub(2) as usize;
    let saved = highlighted_saved_path(state);
    let offset = next_dest_offset(
        state.pointer.dest_follow_path.as_deref(),
        saved.as_deref(),
        &state.pointer.destination_paths,
        state.status_list_state.offset(),
        view_rows,
    );
    *state.status_list_state.offset_mut() = offset;
    let highlight_idx = saved.as_ref().and_then(|path| {
        state
            .pointer
            .destination_paths
            .iter()
            .position(|candidate| candidate == path)
    });
    state.pointer.dest_follow_path = saved;

    let items: Vec<ListItem<'_>> = lines.into_iter().map(ListItem::new).collect();

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(state.theme.border)
            .style(state.theme.body),
    );

    f.render_stateful_widget(list, area, &mut state.status_list_state);
    paint_followed_row(f, state, area, highlight_idx);

    let total = dest_count;
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

fn highlighted_saved_path(state: &AppState) -> Option<PathBuf> {
    let node = state.nodes.get(state.list_state.selected()?)?;
    node.assigned_dest()?;
    Some(node.path.clone())
}

/// Bring `index` into a window of `view_rows` that starts at `offset`.
pub(crate) fn scroll_index_into_view(offset: usize, index: usize, view_rows: usize) -> usize {
    if view_rows == 0 {
        return offset;
    }
    if index < offset {
        index
    } else if index >= offset.saturating_add(view_rows) {
        index.saturating_add(1).saturating_sub(view_rows)
    } else {
        offset
    }
}

pub(crate) fn next_dest_offset(
    previous: Option<&Path>,
    next: Option<&Path>,
    paths: &[PathBuf],
    offset: usize,
    view_rows: usize,
) -> usize {
    if previous == next {
        return offset;
    }
    let Some(path) = next else {
        return offset;
    };
    let Some(index) = paths.iter().position(|candidate| candidate == path) else {
        return offset;
    };
    scroll_index_into_view(offset, index, view_rows)
}

fn paint_followed_row(f: &mut Frame, state: &AppState, area: Rect, highlight_idx: Option<usize>) {
    let Some(idx) = highlight_idx else {
        return;
    };
    let offset = state.status_list_state.offset();
    if idx < offset {
        return;
    }
    let row = (idx - offset) as u16;
    let inner_height = area.height.saturating_sub(2);
    if row >= inner_height || area.width < 2 {
        return;
    }
    let row_area = Rect {
        x: area.x + 1,
        y: area.y + 1 + row,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    f.buffer_mut().set_style(row_area, state.theme.selected);
}

#[cfg(test)]
mod tests {
    use super::{next_dest_offset, scroll_index_into_view};
    use std::path::PathBuf;

    #[test]
    fn scroll_keeps_the_index_inside_the_window() {
        assert_eq!(scroll_index_into_view(4, 1, 5), 1);
        assert_eq!(scroll_index_into_view(4, 6, 5), 4);
        assert_eq!(scroll_index_into_view(0, 7, 5), 3);
        assert_eq!(scroll_index_into_view(2, 9, 0), 2);
    }

    #[test]
    fn follow_scrolls_only_when_the_source_cursor_changes() {
        let paths = vec![
            PathBuf::from(".bashrc"),
            PathBuf::from(".gitconfig"),
            PathBuf::from(".zshrc"),
        ];
        let zsh = PathBuf::from(".zshrc");
        let bash = PathBuf::from(".bashrc");

        assert_eq!(
            next_dest_offset(None, Some(&zsh), &paths, 0, 1),
            2,
            "a new highlight below the fold scrolls to that row"
        );
        assert_eq!(
            next_dest_offset(Some(&zsh), Some(&zsh), &paths, 0, 1),
            0,
            "the same highlight leaves a manual scroll alone"
        );
        assert_eq!(
            next_dest_offset(Some(&zsh), None, &paths, 2, 1),
            2,
            "an unsaved row does not jump the destinations list"
        );
        assert_eq!(next_dest_offset(None, Some(&bash), &paths, 2, 1), 0);
    }
}
