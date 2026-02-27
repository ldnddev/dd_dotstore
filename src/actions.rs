use crate::state::{Action, AppState, BulkAction, Modal, NodeKind};
use crate::tree::{find_mut_node, flatten_visible, set_dest};
use anyhow::{Context, Result, bail};
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

pub fn remove_symlink(state: &mut AppState, rel_src: &Path) -> Result<bool> {
    let mut removed = false;
    let mut old_dest: Option<PathBuf> = None;

    if let Some(node) = find_mut_node(&mut state.tree, rel_src)
        && let NodeKind::File { dest } = &mut node.kind
    {
        old_dest = dest.clone();
        if let Some(dest_path) = dest.as_ref()
            && let Ok(meta) = fs::symlink_metadata(dest_path)
            && meta.file_type().is_symlink()
        {
            fs::remove_file(dest_path)?;
            removed = true;
        }
        *dest = None;
    }

    if removed && let Some(dest) = old_dest {
        push_history(
            state,
            Action::Remove {
                src: rel_src.to_path_buf(),
                dest,
            },
        );
    }

    flatten_visible(state);
    state.update_symlink_statuses()?;
    Ok(removed)
}

pub fn confirm_bulk(state: &mut AppState, action: BulkAction) -> Result<()> {
    let selected_paths: Vec<_> = state
        .nodes
        .iter()
        .filter(|n| n.selected)
        .filter(|n| matches!(n.kind, NodeKind::File { .. }))
        .map(|n| n.path.clone())
        .collect();

    for rel in selected_paths {
        match action {
            BulkAction::Create => {
                let fallback = state.project_root.join(".linked").join(&rel);
                let dest = find_mut_node(&mut state.tree, &rel)
                    .and_then(|n| match &n.kind {
                        NodeKind::File { dest } => dest.clone(),
                        NodeKind::Folder { .. } => None,
                    })
                    .unwrap_or(fallback);
                let _ = create_symlink(state, &rel, &dest)?;
            }
            BulkAction::Remove => {
                let _ = remove_symlink(state, &rel)?;
            }
        }

        if let Some(node) = find_mut_node(&mut state.tree, &rel) {
            node.selected = false;
        }
    }

    flatten_visible(state);
    Ok(())
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
        Action::Remove { src, dest } => {
            let _ = create_symlink(state, &src, &dest)?;
            state.history.pop_back();
        }
    }

    flatten_visible(state);
    state.update_symlink_statuses()?;
    Ok(())
}

pub fn assign_destination(state: &mut AppState, rel_src: &Path, dest: PathBuf) -> Result<()> {
    if let Some(node) = find_mut_node(&mut state.tree, rel_src)
        && let NodeKind::File { dest: current_dest } = &mut node.kind
        && let Some(old_dest) = current_dest.clone()
        && old_dest != dest
        && let Ok(meta) = fs::symlink_metadata(&old_dest)
        && meta.file_type().is_symlink()
    {
        state.modal = Some(Modal::Error {
            msg: format!(
                "Destination is locked by existing symlink: {}. Remove it first with bulk remove (x).",
                old_dest.display()
            ),
        });
        return Ok(());
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
        state.modal = Some(Modal::Error {
            msg: format!("No export directory found at {}", dir.display()),
        });
        return Ok(());
    };

    let mut candidates: Vec<PathBuf> = read_dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();

    candidates.sort();
    if candidates.is_empty() {
        state.modal = Some(Modal::Error {
            msg: format!("No export files found in {}", dir.display()),
        });
        return Ok(());
    }

    state.modal = Some(Modal::ImportPicker {
        files: candidates,
        selected: 0,
    });
    Ok(())
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

    state.modal = Some(Modal::Error {
        msg: format!("Imported {}", import_path.display()),
    });
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

    state.modal = Some(Modal::Error {
        msg: format!("Exported current state to {}", export_path.display()),
    });
    Ok(())
}
