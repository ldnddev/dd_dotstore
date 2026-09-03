use crate::state::{ActionMode, AppState, Node, NodeKind, SymlinkStatus};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn build_tree(root: &Path, ignores: &[String]) -> Vec<Node> {
    fn recurse(dir: &Path, root: &Path, ignores: &[String]) -> Vec<Node> {
        let mut children = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return children;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if ignores
                .iter()
                .any(|pat| name == *pat || name.ends_with(pat))
            {
                continue;
            }

            let path = entry.path();
            let rel_path = match path.strip_prefix(root) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };

            let kind = if entry.file_type().is_ok_and(|t| t.is_dir()) {
                NodeKind::Folder {
                    children: recurse(&path, root, ignores),
                    expanded: false,
                    dest: None,
                }
            } else {
                NodeKind::File { dest: None }
            };

            let node = Node {
                name,
                path: rel_path,
                kind,
                selected: false,
                action_mode: ActionMode::Symlink,
                symlink_status: SymlinkStatus::None,
            };

            if matches!(node.kind, NodeKind::Folder { .. }) {
                dirs.push(node);
            } else {
                files.push(node);
            }
        }

        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));
        children.extend(dirs);
        children.extend(files);
        children
    }

    recurse(root, root, ignores)
}

/// Phase 1 — query non-empty only. Recurse ALL children (even collapsed) to
/// compute keep and set expanded=true on folders that contain a match.
/// Does not emit rows. Returns whether self or a descendant matches.
fn annotate_filter_matches(node: &mut Node, query: &[char], keep: &mut HashSet<PathBuf>) -> bool {
    let self_match = fuzzy_match(&node.name, query);
    let mut child_match = false;
    if let NodeKind::Folder {
        children, expanded, ..
    } = &mut node.kind
    {
        for child in children.iter_mut() {
            if annotate_filter_matches(child, query, keep) {
                child_match = true;
            }
        }
        if child_match {
            *expanded = true;
        }
    }
    let matches = self_match || child_match;
    if matches {
        keep.insert(node.path.clone());
    }
    matches
}

/// Phase 2 — emit pre-order (push the display row, then children).
/// Empty query: walk children only if `expanded`. Emit every visited node,
/// including unmatched folders.
/// Non-empty query: emit iff `keep` contains the path; do not use
/// `match || is Folder`. Ancestors of matches are already expanded.
fn flatten_emit(
    node: &Node,
    depth: usize,
    query: &[char],
    keep: &HashSet<PathBuf>,
    prefix: &str,
    is_last: bool,
    out: &mut Vec<Node>,
) {
    if !query.is_empty() && !keep.contains(&node.path) {
        return;
    }

    let mut display = node.clone();
    // PR 5: set has_configured_descendant from the live node, switch draw to that
    // flag, then children.clear() on the display clone. Do not clear before that.
    let connector = if depth == 0 {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };
    let continuation = if depth == 0 {
        ""
    } else if is_last {
        "   "
    } else {
        "│  "
    };
    let tree_part = format!("{}{}", prefix, connector);
    display.name = format!("{}{}", tree_part, node.name);
    out.push(display);

    let NodeKind::Folder {
        children, expanded, ..
    } = &node.kind
    else {
        return;
    };

    if query.is_empty() && !*expanded {
        return;
    }

    let visible: Vec<&Node> = if query.is_empty() {
        children.iter().collect()
    } else {
        children.iter().filter(|c| keep.contains(&c.path)).collect()
    };
    let next_prefix = format!("{}{}", prefix, continuation);
    for (i, child) in visible.iter().enumerate() {
        let child_is_last = i == visible.len() - 1;
        flatten_emit(
            child,
            depth + 1,
            query,
            keep,
            &next_prefix,
            child_is_last,
            out,
        );
    }
}

pub fn flatten_visible(state: &mut AppState) {
    state.nodes.clear();
    let filter_query: Vec<char> = state
        .filter
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .collect();

    let mut keep = HashSet::new();
    if !filter_query.is_empty() {
        for node in &mut state.tree {
            annotate_filter_matches(node, &filter_query, &mut keep);
        }
    }

    for node in &state.tree {
        flatten_emit(node, 0, &filter_query, &keep, "", true, &mut state.nodes);
    }

    if state.nodes.is_empty() {
        state.list_state.select(None);
    } else if let Some(i) = state.list_state.selected()
        && i >= state.nodes.len()
    {
        state
            .list_state
            .select(Some(state.nodes.len().saturating_sub(1)));
    }
}

fn fuzzy_match(candidate: &str, query: &[char]) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut idx = 0usize;
    for ch in candidate.chars().map(|c| c.to_ascii_lowercase()) {
        if ch == query[idx] {
            idx += 1;
            if idx == query.len() {
                return true;
            }
        }
    }
    false
}

pub fn find_mut_node<'a>(nodes: &'a mut [Node], target: &Path) -> Option<&'a mut Node> {
    for node in nodes {
        if node.path == target {
            return Some(node);
        }
        if let NodeKind::Folder { children, .. } = &mut node.kind
            && let Some(found) = find_mut_node(children, target)
        {
            return Some(found);
        }
    }
    None
}

pub fn find_node<'a>(nodes: &'a [Node], target: &Path) -> Option<&'a Node> {
    for node in nodes {
        if node.path == target {
            return Some(node);
        }
        if let NodeKind::Folder { children, .. } = &node.kind
            && let Some(found) = find_node(children, target)
        {
            return Some(found);
        }
    }
    None
}

pub fn set_dest(nodes: &mut [Node], rel: &Path, dest: Option<PathBuf>) {
    if let Some(node) = find_mut_node(nodes, rel) {
        match &mut node.kind {
            NodeKind::File { dest: node_dest }
            | NodeKind::Folder {
                dest: node_dest, ..
            } => {
                *node_dest = dest;
            }
        }
    }
}

pub fn set_action_mode(nodes: &mut [Node], rel: &Path, action_mode: ActionMode) {
    if let Some(node) = find_mut_node(nodes, rel) {
        node.action_mode = action_mode;
    }
}

/// Walk the live tree using the same rules as save: dests only when `Some`,
/// modes only when not the default LINK.
pub fn collect_assignments(
    tree: &[Node],
) -> (HashMap<String, String>, HashMap<String, ActionMode>) {
    let mut dests = HashMap::new();
    let mut modes = HashMap::new();
    fn walk(
        node: &Node,
        dests: &mut HashMap<String, String>,
        modes: &mut HashMap<String, ActionMode>,
    ) {
        if node.action_mode != ActionMode::Symlink {
            modes.insert(node.path.to_string_lossy().into_owned(), node.action_mode);
        }
        match &node.kind {
            NodeKind::File { dest: Some(d) } | NodeKind::Folder { dest: Some(d), .. } => {
                dests.insert(
                    node.path.to_string_lossy().into_owned(),
                    d.to_string_lossy().into_owned(),
                );
            }
            _ => {}
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            for child in children {
                walk(child, dests, modes);
            }
        }
    }
    for node in tree {
        walk(node, &mut dests, &mut modes);
    }
    (dests, modes)
}

pub fn snapshot_assignments(
    tree: &[Node],
) -> (HashMap<String, String>, HashMap<String, ActionMode>) {
    collect_assignments(tree)
}

pub fn apply_persisted(
    tree: &mut [Node],
    dests: &HashMap<String, String>,
    modes: &HashMap<String, ActionMode>,
) {
    for (rel, dest) in dests {
        set_dest(tree, Path::new(rel), Some(PathBuf::from(dest)));
    }
    for (rel, action_mode) in modes {
        set_action_mode(tree, Path::new(rel), *action_mode);
    }
}

pub enum AssignmentSource {
    /// Snapshot dests + modes from the live tree (reload, ignore-edit).
    LiveTree,
    /// Use state.persisted_symlinks / persisted_modes (init, import).
    Persisted,
}

pub fn rebuild_tree(state: &mut AppState, source: AssignmentSource) -> Result<()> {
    let selected_path = state
        .list_state
        .selected()
        .and_then(|i| state.nodes.get(i))
        .map(|n| n.path.clone());

    let (dests, modes) = match source {
        AssignmentSource::LiveTree => snapshot_assignments(&state.tree),
        AssignmentSource::Persisted => (
            state.persisted_symlinks.clone(),
            state.persisted_modes.clone(),
        ),
    };

    state.tree = build_tree(&state.project_root, &state.ignore_patterns);
    // Folders start collapsed; expand state is not restored across rebuild.
    apply_persisted(&mut state.tree, &dests, &modes);

    // Flatten happens inside update_symlink_statuses; reselect after that.
    state.update_symlink_statuses()?;

    if let Some(path) = selected_path {
        if let Some(idx) = state.nodes.iter().position(|n| n.path == path) {
            state.list_state.select(Some(idx));
        } else if state.nodes.is_empty() {
            state.list_state.select(None);
        } else {
            state.list_state.select(Some(0));
        }
    }
    Ok(())
}

pub fn update_symlink_statuses_recursive(nodes: &mut [Node], root: &Path) {
    for node in nodes {
        match &node.kind {
            NodeKind::File { dest: Some(dest) }
            | NodeKind::Folder {
                dest: Some(dest), ..
            } => {
                node.symlink_status = match node.action_mode {
                    ActionMode::Symlink => match fs::symlink_metadata(dest) {
                        Ok(meta) if meta.file_type().is_symlink() => match fs::read_link(dest) {
                            Ok(target) if target == root.join(&node.path) => SymlinkStatus::Valid,
                            _ => SymlinkStatus::Broken,
                        },
                        Ok(_) => SymlinkStatus::Broken,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                            SymlinkStatus::Planned
                        }
                        Err(_) => SymlinkStatus::Unknown,
                    },
                    ActionMode::Copy => match fs::symlink_metadata(dest) {
                        Ok(meta) if meta.file_type().is_symlink() => SymlinkStatus::Broken,
                        Ok(_) => SymlinkStatus::Valid,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                            SymlinkStatus::Planned
                        }
                        Err(_) => SymlinkStatus::Unknown,
                    },
                };
            }
            NodeKind::File { dest: None } | NodeKind::Folder { dest: None, .. } => {
                node.symlink_status = SymlinkStatus::None;
            }
        }

        if let NodeKind::Folder { children, .. } = &mut node.kind {
            update_symlink_statuses_recursive(children, root);
        }
    }
}

pub fn toggle_expand(state: &mut AppState, rel_path: &Path) {
    if let Some(node) = find_mut_node(&mut state.tree, rel_path)
        && let NodeKind::Folder { expanded, .. } = &mut node.kind
    {
        *expanded = !*expanded;
    }
}

pub fn toggle_selected(state: &mut AppState, rel_path: &Path) {
    if let Some(node) = find_mut_node(&mut state.tree, rel_path) {
        node.selected = !node.selected;
    }
}

/// Expands all ancestor folders for the given relative path so that the target
/// becomes visible in the flattened nodes list. Used for auto-expand on
/// right-panel clicks.
pub fn expand_ancestors(state: &mut AppState, rel_path: &Path) {
    let mut ancestors: Vec<PathBuf> = Vec::new();
    let mut current = rel_path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent == Path::new("") || parent.as_os_str().is_empty() {
            break;
        }
        ancestors.push(parent.to_path_buf());
        current = parent.to_path_buf();
    }
    ancestors.reverse();
    for anc in ancestors {
        if let Some(node) = find_mut_node(&mut state.tree, &anc)
            && let NodeKind::Folder { expanded, .. } = &mut node.kind
        {
            *expanded = true;
        }
    }
}
