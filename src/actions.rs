use crate::state::{Action, ActionMode, AppState, BulkAction, Conflict, Modal, Node, NodeKind};
use crate::toast::ToastLevel;
use crate::tree::{find_mut_node, find_node, flatten_visible, set_action_mode, set_dest};
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

pub fn push_history(state: &mut AppState, action: Action) {
    state.history.push_back(action);
    while state.history.len() > 10 {
        state.history.pop_front();
    }
}

pub fn create_symlink(state: &mut AppState, rel_src: &Path, dest: &Path) -> Result<bool> {
    create_symlink_with_overwrite(state, rel_src, dest, false)
}

pub fn create_copy(state: &mut AppState, rel_src: &Path, dest: &Path) -> Result<bool> {
    create_copy_with_overwrite(state, rel_src, dest, false)
}

fn create_symlink_with_overwrite(
    state: &mut AppState,
    rel_src: &Path,
    dest: &Path,
    overwrite_real_files: bool,
) -> Result<bool> {
    #[cfg(not(unix))]
    {
        let _ = state;
        let _ = rel_src;
        let _ = dest;
        bail!("Symlink creation is only implemented for unix targets");
    }

    #[cfg(unix)]
    {
        let src_abs = state.project_root.join(rel_src);
        if !src_abs.exists() {
            bail!("Source does not exist: {}", src_abs.display());
        }

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        if dest.exists() || fs::symlink_metadata(dest).is_ok() {
            let meta = fs::symlink_metadata(dest)?;
            if meta.file_type().is_symlink() {
                fs::remove_file(dest)?;
            } else if overwrite_real_files {
                if meta.is_dir() {
                    fs::remove_dir_all(dest)?;
                } else {
                    fs::remove_file(dest)?;
                }
            } else {
                return Ok(false);
            }
        }

        unix_fs::symlink(&src_abs, dest).with_context(|| {
            format!(
                "Failed to create symlink {} -> {}",
                dest.display(),
                src_abs.display()
            )
        })?;

        set_dest(&mut state.tree, rel_src, Some(dest.to_path_buf()));
        set_action_mode(&mut state.tree, rel_src, ActionMode::Symlink);
        push_history(
            state,
            Action::Create {
                src: rel_src.to_path_buf(),
                dest: dest.to_path_buf(),
            },
        );

        flatten_visible(state);
        state.update_symlink_statuses()?;
        state.mark_dirty();
        Ok(true)
    }
}

fn create_copy_with_overwrite(
    state: &mut AppState,
    rel_src: &Path,
    dest: &Path,
    overwrite_existing: bool,
) -> Result<bool> {
    let src_abs = state.project_root.join(rel_src);
    if !src_abs.exists() {
        bail!("Source does not exist: {}", src_abs.display());
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }

    if fs::symlink_metadata(dest).is_ok() {
        if !overwrite_existing {
            return Ok(false);
        }
        remove_path_any(dest)?;
    }

    copy_path_recursive(&src_abs, dest)
        .with_context(|| format!("Failed to copy {} to {}", src_abs.display(), dest.display()))?;

    set_dest(&mut state.tree, rel_src, Some(dest.to_path_buf()));
    set_action_mode(&mut state.tree, rel_src, ActionMode::Copy);
    push_history(
        state,
        Action::Copy {
            src: rel_src.to_path_buf(),
            dest: dest.to_path_buf(),
        },
    );

    flatten_visible(state);
    state.update_symlink_statuses()?;
    state.mark_dirty();
    Ok(true)
}

fn copy_path_recursive(src: &Path, dest: &Path) -> Result<()> {
    let meta = fs::metadata(src)?;
    if meta.is_dir() {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_path_recursive(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else {
        let _ = fs::copy(src, dest)?;
    }
    Ok(())
}

fn remove_path_any(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path)?;
    } else if meta.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub fn remove_symlink(state: &mut AppState, rel_src: &Path) -> Result<bool> {
    remove_deployed_item(state, rel_src)
}

fn remove_deployed_item(state: &mut AppState, rel_src: &Path) -> Result<bool> {
    let mut removed = false;
    let mut old_dest: Option<PathBuf> = None;
    let mut removed_mode = ActionMode::Symlink;

    if let Some(node) = find_mut_node(&mut state.tree, rel_src) {
        removed_mode = node.action_mode;
        match &mut node.kind {
            NodeKind::File { dest } | NodeKind::Folder { dest, .. } => {
                // Never guess a dest on remove — dest None is a no-op.
                old_dest = dest.clone();
                if let Some(dest_path) = old_dest.as_ref() {
                    removed = remove_destination_for_mode(dest_path, node.action_mode)?;
                }
                *dest = None;
            }
        }
    }

    if removed && let Some(dest) = old_dest {
        let action = match removed_mode {
            ActionMode::Symlink => Action::Remove {
                src: rel_src.to_path_buf(),
                dest,
            },
            ActionMode::Copy => Action::RemoveCopy {
                src: rel_src.to_path_buf(),
                dest,
            },
        };
        push_history(state, action);
        state.mark_dirty();
    }

    flatten_visible(state);
    state.update_symlink_statuses()?;
    Ok(removed)
}

fn remove_destination_for_mode(dest_path: &Path, action_mode: ActionMode) -> Result<bool> {
    let Ok(meta) = fs::symlink_metadata(dest_path) else {
        return Ok(false);
    };

    if meta.file_type().is_symlink() {
        fs::remove_file(dest_path)?;
        return Ok(true);
    }

    if action_mode == ActionMode::Copy {
        remove_path_any(dest_path)?;
        return Ok(true);
    }

    Ok(false)
}

pub fn default_dest(project_root: &Path, rel: &Path, is_dir: bool) -> PathBuf {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        // Do not target /.bashrc. Unset HOME → last-resort fallback.
        return fallback_linked(project_root, rel);
    };
    let xdg_config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    default_dest_with(&home, &xdg_config, project_root, rel, is_dir)
}

pub fn default_dest_with(
    home: &Path,
    xdg_config: &Path,
    project_root: &Path,
    rel: &Path,
    is_dir: bool,
) -> PathBuf {
    let mut comps = rel.components();
    let Some(first) = comps.next() else {
        return fallback_linked(project_root, rel);
    };
    let first_s = first.as_os_str();

    if first_s == ".config" {
        let rest: PathBuf = comps.collect();
        if rest.as_os_str().is_empty() {
            // Directory named .config → XDG. File named .config → $HOME/.config.
            return if is_dir {
                xdg_config.to_path_buf()
            } else {
                home.join(".config")
            };
        }
        return xdg_config.join(rest);
    }

    let is_single = comps.next().is_none();
    if is_single {
        let name = Path::new(first_s);
        let name_str = name.to_string_lossy();
        if is_dir && !name_str.starts_with('.') {
            return xdg_config.join(name); // nvim/ → ~/.config/nvim
        }
        return home.join(name); // .bashrc, .ssh/, README.md, .config file
    }

    fallback_linked(project_root, rel)
}

fn fallback_linked(project_root: &Path, rel: &Path) -> PathBuf {
    project_root.join(".linked").join(rel)
}

/// Assigned dest if any, else default_dest. Create-path only.
pub fn planned_dest(state: &AppState, node: &Node) -> PathBuf {
    let is_dir = matches!(node.kind, NodeKind::Folder { .. });
    match &node.kind {
        NodeKind::File { dest: Some(d) } | NodeKind::Folder { dest: Some(d), .. } => d.clone(),
        NodeKind::File { dest: None } | NodeKind::Folder { dest: None, .. } => {
            default_dest(&state.project_root, &node.path, is_dir)
        }
    }
}

/// Checkboxes if any, else the highlighted row. Does not flip selection.
pub fn action_targets(state: &AppState) -> Vec<PathBuf> {
    let selected: Vec<_> = state
        .nodes
        .iter()
        .filter(|n| n.selected)
        .map(|n| n.path.clone())
        .collect();
    if !selected.is_empty() {
        return selected;
    }
    state
        .list_state
        .selected()
        .and_then(|i| state.nodes.get(i))
        .map(|n| vec![n.path.clone()])
        .unwrap_or_default()
}

fn action_target_nodes(state: &AppState) -> Vec<&Node> {
    let paths = action_targets(state);
    paths
        .iter()
        .filter_map(|p| state.nodes.iter().find(|n| n.path == *p))
        .collect()
}

fn node_assigned_dest(node: &Node) -> Option<&Path> {
    match &node.kind {
        NodeKind::File { dest } | NodeKind::Folder { dest, .. } => dest.as_deref(),
    }
}

pub fn confirm_bulk(state: &mut AppState, action: BulkAction) -> Result<()> {
    confirm_bulk_with_overwrite(state, action, false)
}

pub fn confirm_bulk_with_overwrite(
    state: &mut AppState,
    action: BulkAction,
    overwrite_real_files: bool,
) -> Result<()> {
    let selected_paths = action_targets(state);

    for rel in selected_paths {
        match action {
            BulkAction::Create => {
                let Some((assigned, is_dir, path, action_mode)) =
                    find_node(&state.tree, &rel).map(|n| {
                        (
                            node_assigned_dest(n).map(Path::to_path_buf),
                            matches!(n.kind, NodeKind::Folder { .. }),
                            n.path.clone(),
                            n.action_mode,
                        )
                    })
                else {
                    continue;
                };
                let dest =
                    assigned.unwrap_or_else(|| default_dest(&state.project_root, &path, is_dir));

                match action_mode {
                    ActionMode::Symlink => {
                        let _ = create_symlink_with_overwrite(
                            state,
                            &rel,
                            &dest,
                            overwrite_real_files,
                        )?;
                    }
                    ActionMode::Copy => {
                        let _ =
                            create_copy_with_overwrite(state, &rel, &dest, overwrite_real_files)?;
                    }
                }
            }
            BulkAction::Remove => {
                let _ = remove_deployed_item(state, &rel)?;
            }
        }

        if let Some(node) = find_mut_node(&mut state.tree, &rel) {
            node.selected = false;
        }
    }

    flatten_visible(state);
    state.persist_now_or_toast();
    Ok(())
}

/// Collect human-readable preview lines for what a bulk action would do.
/// DIR overwrite lines are first; the vec is not truncated (Plan modal scrolls).
pub fn collect_preview_lines(state: &AppState, action: BulkAction) -> Vec<String> {
    let targets = action_target_nodes(state);

    if targets.is_empty() {
        return vec!["(no items selected)".to_string()];
    }

    let mut items: Vec<(bool, String)> = Vec::new();

    for node in targets {
        match action {
            BulkAction::Create => {
                let assigned = node_assigned_dest(node).is_some();
                let dest = planned_dest(state, node);
                let arrow = if node.action_mode == ActionMode::Symlink {
                    "->"
                } else {
                    "=>"
                };
                let mut line = format!(
                    "{} {} {} {}",
                    node.action_mode.label(),
                    node.path.display(),
                    arrow,
                    dest.display()
                );
                let mut is_overwrite_dir = false;
                if let Ok(meta) = fs::symlink_metadata(&dest)
                    && (node.action_mode == ActionMode::Copy || !meta.file_type().is_symlink())
                {
                    if meta.is_dir() {
                        line.push_str("  [OVERWRITE REAL DIR]");
                        is_overwrite_dir = true;
                    } else {
                        line.push_str("  [OVERWRITE REAL FILE]");
                    }
                }
                if !assigned && dest == fallback_linked(&state.project_root, &node.path) {
                    line.push_str("  [fallback: .linked]");
                }
                items.push((is_overwrite_dir, line));
            }
            BulkAction::Remove => {
                if let Some(d) = node_assigned_dest(node) {
                    let arrow = if node.action_mode == ActionMode::Symlink {
                        "->"
                    } else {
                        "=>"
                    };
                    items.push((
                        false,
                        format!(
                            "REMOVE {} {} {} {}",
                            node.action_mode.label(),
                            node.path.display(),
                            arrow,
                            d.display()
                        ),
                    ));
                } else {
                    items.push((false, format!("REMOVE (no dest) {}", node.path.display())));
                }
            }
        }
    }

    items.sort_by_key(|(is_dir, _)| !*is_dir);
    items.into_iter().map(|(_, line)| line).collect()
}

pub fn selected_create_conflicts(state: &AppState) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    let mut seen = HashSet::new();

    for node in action_target_nodes(state) {
        let dest = planned_dest(state, node);

        if seen.contains(&dest) {
            continue;
        }
        if let Ok(meta) = fs::symlink_metadata(&dest)
            && (node.action_mode == ActionMode::Copy || !meta.file_type().is_symlink())
        {
            conflicts.push(Conflict {
                dest: dest.clone(),
                is_real_file: true,
                is_dir: meta.is_dir(),
            });
            seen.insert(dest);
        }
    }

    // Dirs first so every DIR is visible before file rows.
    conflicts.sort_by_key(|c| !c.is_dir);
    conflicts
}

pub fn undo_last(state: &mut AppState) -> Result<()> {
    let Some(action) = state.history.pop_back() else {
        return Ok(());
    };

    match action {
        Action::Create { src, dest } => {
            if let Ok(meta) = fs::symlink_metadata(&dest)
                && meta.file_type().is_symlink()
            {
                fs::remove_file(&dest)?;
            }
            set_dest(&mut state.tree, &src, None);
        }
        Action::Copy { src, dest } => {
            if fs::symlink_metadata(&dest).is_ok() {
                remove_path_any(&dest)?;
            }
            set_dest(&mut state.tree, &src, None);
        }
        Action::Remove { src, dest } => {
            let _ = create_symlink(state, &src, &dest)?;
            state.history.pop_back();
        }
        Action::RemoveCopy { src, dest } => {
            let _ = create_copy(state, &src, &dest)?;
            state.history.pop_back();
        }
    }

    flatten_visible(state);
    state.update_symlink_statuses()?;
    state.mark_dirty();
    state.persist_now_or_toast();
    Ok(())
}

pub fn assign_destination(state: &mut AppState, rel_src: &Path, dest: PathBuf) -> Result<()> {
    if let Some(node) = find_mut_node(&mut state.tree, rel_src) {
        let current_dest = match &mut node.kind {
            NodeKind::File { dest: d } | NodeKind::Folder { dest: d, .. } => d.clone(),
        };
        if let Some(old_dest) = current_dest
            && old_dest != dest
            && let Ok(meta) = fs::symlink_metadata(&old_dest)
            && meta.file_type().is_symlink()
        {
            state.show_toast(
                ToastLevel::Error,
                format!(
                    "Destination is locked by existing symlink: {}. Remove it first with bulk remove (x).",
                    old_dest.display()
                ),
            );
            return Ok(());
        }
    }

    set_dest(&mut state.tree, rel_src, Some(dest));
    flatten_visible(state);
    state.update_symlink_statuses()?;
    state.mark_dirty();
    Ok(())
}

pub fn export_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".dd_dotstore")
        .join("exports")
}

pub fn open_import_picker(state: &mut AppState) -> Result<()> {
    let dir = export_dir();
    let Ok(read_dir) = fs::read_dir(&dir) else {
        state.show_toast(
            ToastLevel::Warning,
            format!("No export directory found at {}", dir.display()),
        );
        return Ok(());
    };

    let candidates = sorted_import_candidates(read_dir);
    if candidates.is_empty() {
        state.show_toast(
            ToastLevel::Warning,
            format!("No export files found in {}", dir.display()),
        );
        return Ok(());
    }

    state.modal = Some(Modal::ImportPicker {
        files: candidates,
        selected: 0,
    });
    Ok(())
}

fn sorted_import_candidates(read_dir: fs::ReadDir) -> Vec<PathBuf> {
    let mut candidates: Vec<(PathBuf, SystemTime)> = read_dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .map(|p| {
            let modified = fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            (p, modified)
        })
        .collect();

    candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    candidates.into_iter().map(|(p, _)| p).collect()
}

pub fn import_from_path(state: &mut AppState, import_path: &Path) -> Result<()> {
    let imported = crate::state::load_persistent(import_path)?;
    state.persisted_symlinks = imported.symlinks;
    state.persisted_modes = imported.modes;
    state.history = imported.history.into_iter().collect();
    crate::tree::rebuild_tree(state, crate::tree::AssignmentSource::Persisted)?;
    state.mark_dirty();
    state.show_toast(
        ToastLevel::Success,
        format!("Imported {}", import_path.display()),
    );
    state.persist_now_or_toast();
    Ok(())
}

pub fn open_export_picker(state: &mut AppState) -> Result<()> {
    let dir = export_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create export directory {}", dir.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    state.modal = Some(Modal::ExportPicker {
        dir,
        filename: format!("dd_dotstore_export_{}.json", stamp),
    });
    Ok(())
}

pub fn export_to_path(state: &mut AppState, export_path: &Path) -> Result<()> {
    crate::state::save(state, export_path)?;
    state.mark_dirty();
    state.show_toast(
        ToastLevel::Success,
        format!("Exported current state to {}", export_path.display()),
    );
    state.persist_now_or_toast();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sorted_import_candidates;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos))
    }

    #[test]
    fn import_candidates_are_sorted_newest_first() {
        let root = temp_path("dd_dotstore_actions_sort");
        fs::create_dir_all(&root).expect("create root");
        let old = root.join("a.json");
        let new = root.join("b.json");
        fs::write(&old, "{}").expect("write old");
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(&new, "{}").expect("write new");

        let read_dir = fs::read_dir(&root).expect("read dir");
        let files = sorted_import_candidates(read_dir);
        assert_eq!(files.first(), Some(&new));
        assert_eq!(files.get(1), Some(&old));

        let _ = fs::remove_dir_all(root);
    }
}
