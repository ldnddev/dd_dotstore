use crate::domain::{ActionMode, AppState, Node, NodeKind};
use ratatui::layout::Rect;
use std::ops::Range;

pub const CHECKBOX_UNCHECKED: &str = "[ ] ";
pub const CHECKBOX_CHECKED: &str = "[✓] ";
pub const ICON_NONE: &str = "  ";
pub const ICON_VALID: &str = "✓ ";
pub const ICON_BROKEN: &str = "✗ ";
pub const ICON_PLANNED: &str = "○ ";
pub const ICON_UNKNOWN: &str = "? ";
pub const ICON_SUBTREE: &str = "◌ ";
pub const BADGE_CONFIGURED: &str = " ●";

/// Same cell draw emits and `source_row_zones` measures. LINK/COPY are both 4 letters.
pub fn mode_label(mode: ActionMode) -> String {
    format!("[{}] ", mode.label())
}

pub struct SourceRowZones {
    pub checkbox: Range<u16>, // "[ ] " / "[✓] "
    pub icon: Range<u16>,
    pub mode: Range<u16>, // "[LINK] "
    pub tree: Range<u16>, // connector only, folders
    pub name: Range<u16>, // name + optional planned dest suffix
}

fn str_width(s: &str) -> u16 {
    s.chars().count() as u16
}

/// Widths come from the same strings draw uses, not magic columns.
pub fn source_row_zones(
    tree_prefix: &str,
    name: &str,
    planned_dest_suffix: Option<&str>,
) -> SourceRowZones {
    let mut x = 0u16;
    let checkbox_w = str_width(CHECKBOX_UNCHECKED);
    let checkbox = x..(x + checkbox_w);
    x += checkbox_w;

    let icon_w = str_width(ICON_VALID);
    let icon = x..(x + icon_w);
    x += icon_w;

    let mode_w = str_width(&mode_label(ActionMode::Symlink));
    let mode = x..(x + mode_w);
    x += mode_w;

    let tree_w = str_width(tree_prefix);
    let tree = x..(x + tree_w);
    x += tree_w;

    let mut name_w = str_width(name);
    if let Some(suffix) = planned_dest_suffix {
        name_w += str_width(suffix);
    }
    let name = x..(x + name_w);

    SourceRowZones {
        checkbox,
        icon,
        mode,
        tree,
        name,
    }
}

/// Flatten bakes the tree connector into `node.name`. Recover the pieces so
/// draw and hit-test share source_row_zones.
pub fn split_tree_prefix(node: &Node) -> (&str, &str) {
    let basename = node
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(node.name.as_str());
    if let Some(prefix) = node.name.strip_suffix(basename) {
        (prefix, basename)
    } else {
        ("", node.name.as_str())
    }
}

pub fn truncate_ellipsis(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let take = max.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
}

pub fn display_name_and_badge(node: &Node) -> (String, String) {
    let (tree_prefix, basename) = split_tree_prefix(node);
    let mut name = basename.to_string();
    if matches!(&node.kind, NodeKind::Folder { dest: None, .. }) && node.has_configured_descendant {
        name.push_str(BADGE_CONFIGURED);
    }
    (tree_prefix.to_string(), name)
}

pub fn planned_suffix_for_row(state: &AppState, node: &Node, inner_width: u16) -> Option<String> {
    let dest_none = matches!(
        &node.kind,
        NodeKind::File { dest: None } | NodeKind::Folder { dest: None, .. }
    );
    if !dest_none {
        return None;
    }
    let (tree_prefix, name) = display_name_and_badge(node);
    let planned = crate::actions::planned_dest(state, node);
    let raw = format!(" → {}", planned.display());
    let without = source_row_zones(&tree_prefix, &name, None);
    let max_suffix = (inner_width as usize).saturating_sub(without.name.end as usize);
    if max_suffix == 0 {
        None
    } else {
        Some(truncate_ellipsis(&raw, max_suffix))
    }
}

pub fn zones_for_node(state: &AppState, node: &Node) -> SourceRowZones {
    let inner = state.pointer.source_area.width.saturating_sub(2);
    let (tree_prefix, name) = display_name_and_badge(node);
    let suffix = planned_suffix_for_row(state, node, inner);
    source_row_zones(&tree_prefix, &name, suffix.as_deref())
}

pub fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x && col < area.x + area.width && row >= area.y && row < area.y + area.height
}

pub fn is_over_scrollbar(area: &Rect, col: u16, row: u16) -> bool {
    if area.width < 2 {
        return false;
    }
    let sb_col = area.x + area.width - 1;
    col == sb_col && row >= area.y && row < area.y + area.height
}

/// Returns (row index, rel_x) for a click inside the source list.
pub fn hit_test_source_row(state: &AppState, col: u16, row: u16) -> Option<(usize, u16)> {
    if !rect_contains(state.pointer.source_area, col, row) || state.nodes.is_empty() {
        return None;
    }
    let inner_x = state.pointer.source_area.x + 1;
    let inner_y = state.pointer.source_area.y + 1;
    if row < inner_y {
        return None;
    }
    let rel_y = row - inner_y;
    let rel_x = col.saturating_sub(inner_x);
    let offset = state.list_state.offset();
    let idx = offset + rel_y as usize;
    if idx >= state.nodes.len() {
        return None;
    }
    Some((idx, rel_x))
}
