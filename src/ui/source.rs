use crate::domain::{ActionMode, AppState, NodeKind, SymlinkStatus};
use crate::input::hit_test::{
    CHECKBOX_CHECKED, CHECKBOX_UNCHECKED, ICON_BROKEN, ICON_NONE, ICON_PLANNED, ICON_SUBTREE,
    ICON_UNKNOWN, ICON_VALID, display_name_and_badge, mode_label, planned_suffix_for_row,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// Left-truncate overflow so the title prefix and trailing cursor/count stay visible.
pub(crate) fn source_panel_title(
    filter: &str,
    editing: bool,
    selected_count: usize,
    total: usize,
    inner_width: usize,
) -> String {
    let count = if selected_count > 0 {
        format!("[{selected_count} selected / {total}]")
    } else {
        format!("[{total}]")
    };
    if !editing && filter.is_empty() {
        return format!("Source  {count}");
    }

    let cursor = if editing { "█" } else { "" };
    let prefix = "Source (filter: ";
    let suffix = format!("{cursor})  {count}");
    let full_len = prefix.chars().count() + filter.chars().count() + suffix.chars().count();
    if inner_width == 0 || full_len <= inner_width {
        return format!("{prefix}{filter}{suffix}");
    }

    let ellipsis = "…";
    let budget = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(suffix.chars().count())
        .saturating_sub(ellipsis.chars().count());
    let tail: String = filter
        .chars()
        .rev()
        .take(budget)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}{ellipsis}{tail}{suffix}")
}

pub(crate) fn draw_source_panel(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.pointer.source_area = area;
    let inner_width = area.width.saturating_sub(2);
    let items: Vec<ListItem<'_>> = state
        .nodes
        .iter()
        .map(|node| {
            let mut icon = match node.symlink_status {
                SymlinkStatus::Valid => ICON_VALID,
                SymlinkStatus::Broken => ICON_BROKEN,
                SymlinkStatus::Planned => ICON_PLANNED,
                SymlinkStatus::Unknown => ICON_UNKNOWN,
                SymlinkStatus::None => ICON_NONE,
            };
            let mut icon_style = match node.symlink_status {
                SymlinkStatus::Valid => state.theme.valid,
                SymlinkStatus::Broken => state.theme.broken,
                SymlinkStatus::Planned => state.theme.info,
                SymlinkStatus::Unknown => state.theme.highlight,
                SymlinkStatus::None => state.theme.normal,
            };
            if matches!(&node.kind, NodeKind::Folder { dest: None, .. })
                && node.has_configured_descendant
                && node.symlink_status == SymlinkStatus::None
            {
                icon = ICON_SUBTREE;
                icon_style = state.theme.secondary;
            }

            let prefix = if node.selected {
                CHECKBOX_CHECKED
            } else {
                CHECKBOX_UNCHECKED
            };
            let mode_style = match node.action_mode {
                ActionMode::Symlink => state.theme.valid,
                ActionMode::Copy => state.theme.highlight,
            };

            let (tree_prefix, name) = display_name_and_badge(node);
            let name_style = match &node.kind {
                NodeKind::Folder { .. } => state.theme.folder,
                _ => state.theme.file,
            };
            // Keep tree connector + name in one span so styling matches 1.1.
            let label = format!("{tree_prefix}{name}");

            let mut spans = vec![
                Span::styled(prefix, state.theme.normal),
                Span::styled(icon, icon_style),
                Span::styled(mode_label(node.action_mode), mode_style),
                Span::styled(label, name_style),
            ];
            if let Some(suffix) = planned_suffix_for_row(state, node, inner_width) {
                spans.push(Span::styled(suffix, state.theme.secondary));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let total = state.nodes.len();
    let selected_count = state.nodes.iter().filter(|n| n.selected).count();
    let title = source_panel_title(
        &state.filter,
        state.filter_editing,
        selected_count,
        total,
        inner_width as usize,
    );

    let list = List::new(items)
        .block({
            let block = Block::default()
                .borders(Borders::ALL)
                .style(state.theme.body);
            if state.filter_editing {
                block
                    .title(Span::styled(title, state.theme.input_text_focus))
                    .border_style(state.theme.input_border_focus)
            } else {
                block.title(title).border_style(state.theme.active_border)
            }
        })
        .highlight_style(state.theme.selected);

    f.render_stateful_widget(list, area, &mut state.list_state);

    let total = state.nodes.len();
    let view_rows = area.height.saturating_sub(2) as usize;
    if total > view_rows && view_rows > 0 {
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(state.list_state.offset())
            .viewport_content_length(view_rows);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .thumb_style(state.theme.scrollbar);
        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
