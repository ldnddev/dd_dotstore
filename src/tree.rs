use crate::state::{ActionMode, AppState, Node, NodeKind, SymlinkStatus};
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

pub fn flatten_visible(state: &mut AppState) {
    state.nodes.clear();
    let filter_query: Vec<char> = state
        .filter
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .collect();

    fn walk(node: &Node, depth: usize, query: &[char], out: &mut Vec<Node>) {
        let matches = query.is_empty() || fuzzy_match(&node.name, query);
        if matches || matches!(node.kind, NodeKind::Folder { .. }) {
            let mut display = node.clone();
            display.name = format!("{}{}", "  ".repeat(depth), node.name);
            out.push(display);
        }

        if let NodeKind::Folder {
            children, expanded, ..
        } = &node.kind
            && *expanded
        {
            for child in children {
                walk(child, depth + 1, query, out);
            }
        }
    }

    for node in &state.tree {
        walk(node, 0, &filter_query, &mut state.nodes);
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
                        Err(_) => SymlinkStatus::Unknown,
                    },
                    ActionMode::Copy => match fs::symlink_metadata(dest) {
                        Ok(meta) if meta.file_type().is_symlink() => SymlinkStatus::Broken,
                        Ok(_) => SymlinkStatus::Valid,
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
