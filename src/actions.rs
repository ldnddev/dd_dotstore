use crate::state::{Action, ActionMode, AppState, BulkAction, Conflict, Modal, NodeKind};
use crate::toast::ToastLevel;
use crate::tree::{find_mut_node, flatten_visible, set_action_mode, set_dest};
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
            NodeKind::File { dest } => {
                old_dest = dest
                    .clone()
                    .or_else(|| Some(state.project_root.join(".linked").join(rel_src)));
                if let Some(dest_path) = old_dest.as_ref() {
                    removed = remove_destination_for_mode(dest_path, node.action_mode)?;
                }
                *dest = None;
            }
            NodeKind::Folder { dest, .. } => {
                old_dest = dest
                    .clone()
                    .or_else(|| Some(state.project_root.join(".linked").join(rel_src)));
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

pub fn confirm_bulk(state: &mut AppState, action: BulkAction) -> Result<()> {
    confirm_bulk_with_overwrite(state, action, false)
}

pub fn confirm_bulk_with_overwrite(
    state: &mut AppState,
    action: BulkAction,
    overwrite_real_files: bool,
) -> Result<()> {
    let selected_paths: Vec<_> = state
        .nodes
        .iter()
        .filter(|n| n.selected)
        .map(|n| n.path.clone())
        .collect();

    for rel in selected_paths {
        match action {
            BulkAction::Create => {
                let fallback = state.project_root.join(".linked").join(&rel);
                let (dest, action_mode) = find_mut_node(&mut state.tree, &rel)
                    .and_then(|n| match &n.kind {
                        NodeKind::File { dest } => Some((dest.clone(), n.action_mode)),
                        NodeKind::Folder { dest, .. } => Some((dest.clone(), n.action_mode)),
                    })
                    .map(|(dest, action_mode)| (dest.unwrap_or(fallback.clone()), action_mode))
                    .unwrap_or((fallback, ActionMode::Symlink));

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
    Ok(())
}

pub fn selected_create_conflicts(state: &AppState) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    let mut seen = HashSet::new();

    for node in state.nodes.iter().filter(|n| n.selected) {
        let dest = match &node.kind {
            NodeKind::File { dest: Some(d) } => d.clone(),
            NodeKind::File { dest: None } => state.project_root.join(".linked").join(&node.path),
            NodeKind::Folder { dest: Some(d), .. } => d.clone(),
            NodeKind::Folder { dest: None, .. } => {
                state.project_root.join(".linked").join(&node.path)
            }
        };

        if seen.contains(&dest) {
            continue;
        }
        if let Ok(meta) = fs::symlink_metadata(&dest)
            && (node.action_mode == ActionMode::Copy || !meta.file_type().is_symlink())
        {
            conflicts.push(Conflict {
                dest: dest.clone(),
                is_real_file: true,
            });
            seen.insert(dest);
        }
    }

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
    let imported = crate::state::load(import_path)?;
    state.persisted_symlinks = imported.persisted_symlinks;
    state.history = imported.history;

    state.tree = crate::tree::build_tree(&state.project_root, &state.ignore_patterns);
    for (rel, dest) in state.persisted_symlinks.clone() {
        set_dest(&mut state.tree, Path::new(&rel), Some(dest.into()));
    }
    flatten_visible(state);
    state.update_symlink_statuses()?;

    state.show_toast(
        ToastLevel::Success,
        format!("Imported {}", import_path.display()),
    );
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
    crate::state::save(state, &state.config_path)?;

    state.show_toast(
        ToastLevel::Success,
        format!("Exported current state to {}", export_path.display()),
    );
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
