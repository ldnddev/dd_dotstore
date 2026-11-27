//! Rows both the window and `visible_text` paint. Derived from `AppState`.

use dd_dotstore::actions::{self, planned_dest};
use dd_dotstore::domain::{ActionMode, AppState, BulkAction, Modal, Node, NodeKind, SymlinkStatus};
use dd_dotstore::input::hit_test::display_name_and_badge;
use dd_dotstore::toast::{Toast, ToastLevel};

pub struct ModalRow {
    pub label: String,
    pub selected: bool,
    /// Set when a click should move the dialog highlight to this row.
    pub index: Option<usize>,
}

pub struct ModalView {
    pub kind: &'static str,
    pub title: String,
    pub rows: Vec<ModalRow>,
    pub hint: String,
}

pub fn source_line(node: &Node, highlighted: bool) -> String {
    let mark = if highlighted { ">" } else { " " };
    let check = if node.selected { "[✓]" } else { "[ ]" };
    let kind = match node.kind {
        NodeKind::Folder { .. } => "folder",
        NodeKind::File { .. } => "file",
    };
    let (prefix, name) = display_name_and_badge(node);
    format!(
        "{mark} {check} {} [{}] {prefix}{name} {kind}",
        status_icon(node),
        node.action_mode.label()
    )
}

pub fn status_icon(node: &Node) -> &'static str {
    if matches!(&node.kind, NodeKind::Folder { dest: None, .. })
        && node.has_configured_descendant
        && node.symlink_status == SymlinkStatus::None
    {
        return "◌";
    }
    match node.symlink_status {
        SymlinkStatus::Valid => "✓",
        SymlinkStatus::Broken => "✗",
        SymlinkStatus::Planned => "○",
        SymlinkStatus::Unknown => "?",
        SymlinkStatus::None => "·",
    }
}

pub fn focus_line(state: &AppState) -> String {
    let Some(idx) = state.list_state.selected() else {
        return "focus:".to_string();
    };
    let Some(node) = state.nodes.get(idx) else {
        return "focus:".to_string();
    };
    let group = node
        .group
        .as_ref()
        .map(|group| format!(" ·{group}"))
        .unwrap_or_default();
    let dest = planned_dest(state, node);
    format!(
        "focus: {}  {}{}  →  {}  {}",
        node.action_mode.label(),
        node.path.display(),
        group,
        dest.display(),
        focus_status(node)
    )
}

pub fn focus_status(node: &Node) -> &'static str {
    if node.assigned_dest().is_none() {
        return "suggested (not saved)";
    }
    match (node.symlink_status, node.action_mode) {
        (SymlinkStatus::Valid, ActionMode::Copy) => "copied",
        (SymlinkStatus::Valid, ActionMode::Symlink) => "linked",
        (SymlinkStatus::Broken, _) => "broken",
        (SymlinkStatus::Planned, _) => "planned",
        (SymlinkStatus::Unknown | SymlinkStatus::None, _) => "unknown",
    }
}

/// Saved destinations only, in tree order. Same set the terminal pane lists.
pub fn saved_destinations(state: &AppState) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    fn walk(node: &Node, out: &mut Vec<(String, std::path::PathBuf)>) {
        if let Some(dest) = node.assigned_dest() {
            let arrow = match node.action_mode {
                ActionMode::Symlink => "->",
                ActionMode::Copy => "=>",
            };
            let group = node
                .group
                .as_ref()
                .map(|group| format!(" ·{group}"))
                .unwrap_or_default();
            out.push((
                format!(
                    "dest: {} {}{} {} {}",
                    node.action_mode.label(),
                    node.path.display(),
                    group,
                    arrow,
                    dest.display()
                ),
                node.path.clone(),
            ));
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            for child in children {
                walk(child, out);
            }
        }
    }
    for node in &state.tree {
        walk(node, &mut out);
    }
    out
}

pub fn suggestion_suffix(state: &AppState, node: &Node) -> String {
    if node.assigned_dest().is_some() {
        return String::new();
    }
    format!(" → {}", planned_dest(state, node).display())
}

pub fn toast_line(toast: &Toast) -> String {
    let level = match toast.level {
        ToastLevel::Info => "Info",
        ToastLevel::Success => "Success",
        ToastLevel::Warning => "Warning",
        ToastLevel::Error => "Error",
    };
    format!("toast:{level}:{}", toast.message)
}

pub fn modal_view(state: &AppState) -> Option<ModalView> {
    let modal = state.modal.as_ref()?;
    Some(match modal {
        Modal::Plan { action, .. } => {
            let title = match action {
                BulkAction::Create => "Plan: Apply",
                BulkAction::Remove => "Plan: Remove",
            };
            let rows = actions::collect_preview_lines(state, *action)
                .into_iter()
                .map(|label| ModalRow {
                    label,
                    selected: false,
                    index: None,
                })
                .collect();
            ModalView {
                kind: "plan",
                title: title.to_string(),
                rows,
                hint: "Y apply / other cancel   j/k scroll".to_string(),
            }
        }
        Modal::OverwriteWarning { conflicts, .. } => {
            let (dirs, files, total) = dd_dotstore::ui::overwrite_warning_counts(conflicts);
            ModalView {
                kind: "overwrite",
                title: format!("Overwrite Warning  ({dirs} dirs, {files} files, {total} total)"),
                rows: conflicts
                    .iter()
                    .map(|conflict| ModalRow {
                        label: dd_dotstore::ui::overwrite_conflict_row(conflict),
                        selected: false,
                        index: None,
                    })
                    .collect(),
                hint: "These real paths will be replaced. Y apply / other cancel".to_string(),
            }
        }
        Modal::EditDest { browser, .. } => {
            let filtered = browser.filtered_indices();
            let rows = filtered
                .iter()
                .enumerate()
                .filter_map(|(index, entry_idx)| {
                    let entry = browser.entries.get(*entry_idx)?;
                    let suffix = if entry.is_dir { "/" } else { "" };
                    Some(ModalRow {
                        label: format!("{}{suffix}", entry.name),
                        selected: index == browser.selected,
                        index: Some(index),
                    })
                })
                .collect();
            let filter = if browser.filter.is_empty() {
                "<none>"
            } else {
                browser.filter.as_str()
            };
            ModalView {
                kind: "dest",
                title: format!(
                    "Edit Destination: {} | filter: {filter}",
                    browser.current.display()
                ),
                rows,
                hint: "Enter open/assign   Ctrl+S assign this directory   Esc cancel".to_string(),
            }
        }
        Modal::IgnoreEditor { selected, draft } => {
            let mut rows = Vec::new();
            if state.ignore_patterns.is_empty() {
                rows.push(ModalRow {
                    label: "(no patterns — all names visible)".to_string(),
                    selected: false,
                    index: None,
                });
            } else {
                for (index, pat) in state.ignore_patterns.iter().enumerate() {
                    rows.push(ModalRow {
                        label: pat.clone(),
                        selected: index == *selected,
                        index: Some(index),
                    });
                }
            }
            rows.push(ModalRow {
                label: format!("Add: {draft}"),
                selected: false,
                index: None,
            });
            ModalView {
                kind: "ignore",
                title: "Ignore Editor".to_string(),
                rows,
                hint: "j/k move   d/Delete remove   Enter add   Esc close".to_string(),
            }
        }
        Modal::ImportPicker { files, selected } => ModalView {
            kind: "import",
            title: "Import Picker".to_string(),
            rows: files
                .iter()
                .enumerate()
                .map(|(index, path)| {
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    ModalRow {
                        label: format!("{name}  {}", path.display()),
                        selected: index == *selected,
                        index: Some(index),
                    }
                })
                .collect(),
            hint: "Enter import   Esc cancel".to_string(),
        },
        Modal::ExportPicker { dir, filename } => ModalView {
            kind: "export",
            title: "Export Picker".to_string(),
            rows: vec![
                ModalRow {
                    label: format!("Directory: {}", dir.display()),
                    selected: false,
                    index: None,
                },
                ModalRow {
                    label: format!("Filename: {filename}"),
                    selected: false,
                    index: None,
                },
            ],
            hint: "Type to edit, Enter to export, Esc to cancel.".to_string(),
        },
        Modal::GroupEditor { paths, draft } => ModalView {
            kind: "group",
            title: "Group".to_string(),
            rows: vec![
                ModalRow {
                    label: format!("Items: {}", paths.len()),
                    selected: false,
                    index: None,
                },
                ModalRow {
                    label: format!("Group: {draft}"),
                    selected: false,
                    index: None,
                },
            ],
            hint: "Enter save (empty clears). Esc cancel.".to_string(),
        },
        Modal::Adopt {
            candidates,
            selected,
            checked,
        } => ModalView {
            kind: "adopt",
            title: "Adopt project symlinks".to_string(),
            rows: candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let mark = if checked.get(index).copied().unwrap_or(false) {
                        "[✓]"
                    } else {
                        "[ ]"
                    };
                    ModalRow {
                        label: format!(
                            "{mark} {} -> {}",
                            candidate.src.display(),
                            candidate.dest.display()
                        ),
                        selected: index == *selected,
                        index: Some(index),
                    }
                })
                .collect(),
            hint: "Space toggle   a all   Y adopt   Esc cancel".to_string(),
        },
        Modal::Doctor {
            findings, selected, ..
        } => ModalView {
            kind: "doctor",
            title: format!("Doctor ({})", findings.len()),
            rows: findings
                .iter()
                .enumerate()
                .map(|(index, finding)| ModalRow {
                    label: finding.line(),
                    selected: index == *selected,
                    index: Some(index),
                })
                .collect(),
            hint: "Enter jump to source   A adopt orphans   Esc close".to_string(),
        },
    })
}

#[cfg(test)]
pub fn modal_dump(view: &ModalView) -> Vec<String> {
    let mut lines = vec![format!("modal:{}", view.kind), view.title.clone()];
    for row in &view.rows {
        let mark = if row.selected { ">" } else { " " };
        lines.push(format!("{mark} {}", row.label));
    }
    lines.push(view.hint.clone());
    lines
}

/// Zones used by the terminal hit test, re-exported so a row click can share names.
pub fn row_has_children(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Folder { .. })
}

pub fn folder_expanded(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Folder { expanded: true, .. })
}
