use crate::domain::{
    Action, ActionMode, AppState, BulkAction, Conflict, HISTORY_CAP, Modal, Node, NodeKind,
};
use crate::scan::{adopt_candidates_for_state, collect_doctor_findings};
use crate::theme::{
    COLOR_FIELDS, Theme, ThemeColors, ThemeEditor, ThemeSaveTarget, ThemeSource, ThemeStatus,
    color_to_hex, default_config_home, local_theme_path, nudge_channel, parse_hex_input,
    save_theme,
};
use crate::tree::{
    expand_ancestors, find_mut_node, find_node, flatten_visible, set_action_mode, set_dest,
};
use crate::ui::toast::ToastLevel;
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

pub fn push_history(state: &mut AppState, action: Action) {
    state.history.push_back(action);
    while state.history.len() > HISTORY_CAP {
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
                if let Some(dest_path) = dest.as_ref() {
                    removed = remove_destination_for_mode(dest_path, node.action_mode)?;
                }
                old_dest = dest.take();
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
    let selected: Vec<_> = state.nodes.iter().filter(|n| n.selected).collect();
    if !selected.is_empty() {
        return selected;
    }
    state
        .list_state
        .selected()
        .and_then(|i| state.nodes.get(i))
        .map(|n| vec![n])
        .unwrap_or_default()
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
                let group = node
                    .group
                    .as_ref()
                    .map(|g| format!(" [{g}]"))
                    .unwrap_or_default();
                let mut line = format!(
                    "{} {}{} {} {}",
                    node.action_mode.label(),
                    node.path.display(),
                    group,
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
    let imported = crate::domain::load_persistent(import_path)?;
    state.persisted_symlinks = imported.symlinks;
    state.persisted_modes = imported.modes;
    state.persisted_groups = imported.groups;
    state.history = imported.history.into_iter().collect();
    // 1.1 JSON without the key must not clobber session ignores.
    if let Some(pats) = imported.ignore_patterns {
        state.ignore_patterns = pats;
    }
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
    crate::domain::save(state, export_path)?;
    state.mark_dirty();
    state.show_toast(
        ToastLevel::Success,
        format!("Exported current state to {}", export_path.display()),
    );
    state.persist_now_or_toast();
    Ok(())
}

pub fn open_group_editor(state: &mut AppState) {
    let paths = action_targets(state);
    if paths.is_empty() {
        return;
    }
    let mut names: Vec<String> = paths
        .iter()
        .filter_map(|p| find_node(&state.tree, p).and_then(|n| n.group.clone()))
        .collect();
    names.sort();
    names.dedup();
    let draft = if names.len() == 1 {
        names[0].clone()
    } else {
        String::new()
    };
    state.modal = Some(Modal::GroupEditor { paths, draft });
}

pub fn apply_group_name(state: &mut AppState, paths: &[PathBuf], draft: &str) {
    let group = {
        let trimmed = draft.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    let mut changed = false;
    for path in paths {
        if let Some(node) = find_mut_node(&mut state.tree, path)
            && node.group != group
        {
            node.group = group.clone();
            changed = true;
        }
    }
    if changed {
        state.mark_dirty();
        flatten_visible(state);
        state.persist_now_or_toast();
        let msg = match &group {
            Some(name) => format!("Grouped {} item(s) as {name}", paths.len()),
            None => format!("Cleared group on {} item(s)", paths.len()),
        };
        state.show_toast(ToastLevel::Success, msg);
    }
}

pub fn select_group_of_highlight(state: &mut AppState) {
    let Some(idx) = state.list_state.selected() else {
        return;
    };
    let Some(group) = state.nodes.get(idx).and_then(|n| n.group.clone()) else {
        state.show_toast(ToastLevel::Info, "No group on this item");
        return;
    };
    let count = select_nodes_in_group(&mut state.tree, &group);
    flatten_visible(state);
    state.show_toast(
        ToastLevel::Info,
        format!("Selected {count} item(s) in group {group}"),
    );
}

fn select_nodes_in_group(nodes: &mut [Node], group: &str) -> usize {
    let mut count = 0;
    for node in nodes {
        if node.group.as_deref() == Some(group) {
            node.selected = true;
            count += 1;
        }
        if let NodeKind::Folder { children, .. } = &mut node.kind {
            count += select_nodes_in_group(children, group);
        }
    }
    count
}

pub fn open_adopt_picker(state: &mut AppState) {
    let candidates = adopt_candidates_for_state(state);
    if candidates.is_empty() {
        state.show_toast(
            ToastLevel::Info,
            "No project symlinks found under $HOME / XDG that are missing from config",
        );
        return;
    }
    let checked = vec![true; candidates.len()];
    state.modal = Some(Modal::Adopt {
        candidates,
        selected: 0,
        checked,
    });
}

pub fn apply_adopt_candidates(state: &mut AppState, candidates: &[crate::domain::AdoptCandidate]) {
    let mut adopted = 0usize;
    for candidate in candidates {
        if find_node(&state.tree, &candidate.src).is_none() {
            continue;
        }
        set_dest(
            &mut state.tree,
            &candidate.src,
            Some(candidate.dest.clone()),
        );
        set_action_mode(&mut state.tree, &candidate.src, ActionMode::Symlink);
        adopted += 1;
    }
    if adopted == 0 {
        state.show_toast(ToastLevel::Warning, "Nothing to adopt");
        return;
    }
    flatten_visible(state);
    if let Err(err) = state.update_symlink_statuses() {
        state.show_toast(ToastLevel::Error, format!("Status refresh failed: {err}"));
    }
    state.mark_dirty();
    state.persist_now_or_toast();
    state.show_toast(ToastLevel::Success, format!("Adopted {adopted} mapping(s)"));
}

pub fn open_doctor(state: &mut AppState) {
    let findings = collect_doctor_findings(state);
    if findings.is_empty() {
        state.show_toast(ToastLevel::Success, "Doctor: no issues found");
        return;
    }
    state.modal = Some(Modal::Doctor {
        findings,
        selected: 0,
        scroll: 0,
    });
}

pub fn jump_to_doctor_source(state: &mut AppState, src: &Path) {
    expand_ancestors(state, src);
    flatten_visible(state);
    if let Some(idx) = state.nodes.iter().position(|n| n.path == src) {
        state.list_state.select(Some(idx));
    }
}

pub fn open_theme_editor(state: &mut AppState) {
    state.modal = Some(Modal::ThemeEditor(ThemeEditor::from_theme(&state.theme)));
}

pub fn apply_theme_colors(state: &mut AppState, colors: ThemeColors) {
    let quotes = state.theme.header_quotes.clone();
    let source = state.theme.source;
    let version = state.theme.version;
    state.theme = Theme::from_colors(colors, source, version);
    state.theme.header_quotes = quotes;
}

pub fn revert_theme_editor(state: &mut AppState) {
    let Some((colors, quotes, source, version)) = theme_editor_snapshot(state) else {
        return;
    };
    state.theme = Theme::from_colors(colors, source, version);
    state.theme.header_quotes = quotes;
}

fn theme_editor_snapshot(state: &AppState) -> Option<(ThemeColors, Vec<String>, ThemeSource, u64)> {
    match &state.modal {
        Some(Modal::ThemeEditor(editor)) => Some((
            editor.snapshot_colors,
            editor.snapshot_quotes.clone(),
            editor.snapshot_source,
            editor.snapshot_version,
        )),
        _ => None,
    }
}

pub fn theme_editor_select(state: &mut AppState, selected: usize) {
    let colors = state.theme.colors;
    let Some(Modal::ThemeEditor(editor)) = &mut state.modal else {
        return;
    };
    let selected = selected.min(COLOR_FIELDS.len().saturating_sub(1));
    editor.selected = selected;
    if selected < editor.scroll {
        editor.scroll = selected;
    }
    if selected > editor.scroll + 12 {
        editor.scroll = selected.saturating_sub(12);
    }
    if !editor.editing_hex {
        editor.hex_draft = color_to_hex(
            colors
                .get(COLOR_FIELDS[selected].key)
                .unwrap_or(ratatui::style::Color::Black),
        );
    }
}

pub fn theme_editor_nudge(state: &mut AppState, delta: i16) {
    let Some((key, channel)) = theme_editor_key_channel(state) else {
        return;
    };
    let current = state
        .theme
        .colors
        .get(&key)
        .unwrap_or(ratatui::style::Color::Black);
    let next = nudge_channel(current, channel, delta);
    let mut colors = state.theme.colors;
    colors.set(&key, next);
    apply_theme_colors(state, colors);
    if let Some(Modal::ThemeEditor(editor)) = &mut state.modal {
        editor.hex_draft = color_to_hex(next);
        editor.editing_hex = false;
    }
}

fn theme_editor_key_channel(state: &AppState) -> Option<(String, usize)> {
    match &state.modal {
        Some(Modal::ThemeEditor(editor)) => {
            Some((editor.selected_key().to_string(), editor.channel))
        }
        _ => None,
    }
}

pub fn theme_editor_commit_hex(state: &mut AppState) -> Result<()> {
    let Some((key, draft)) = theme_editor_key_draft(state) else {
        return Ok(());
    };
    let color = parse_hex_input(&draft)?;
    let mut colors = state.theme.colors;
    colors.set(&key, color);
    apply_theme_colors(state, colors);
    if let Some(Modal::ThemeEditor(editor)) = &mut state.modal {
        editor.hex_draft = color_to_hex(color);
        editor.editing_hex = false;
    }
    Ok(())
}

fn theme_editor_key_draft(state: &AppState) -> Option<(String, String)> {
    match &state.modal {
        Some(Modal::ThemeEditor(editor)) => {
            Some((editor.selected_key().to_string(), editor.hex_draft.clone()))
        }
        _ => None,
    }
}

pub fn theme_editor_reset(state: &mut AppState) {
    let quotes = state.theme.header_quotes.clone();
    let source = state.theme.source;
    let version = state.theme.version;
    state.theme = Theme::from_colors(ThemeColors::default(), source, version);
    state.theme.header_quotes = quotes;
    if let Some(Modal::ThemeEditor(editor)) = &mut state.modal {
        editor.hex_draft = color_to_hex(
            state
                .theme
                .colors
                .get(editor.selected_key())
                .unwrap_or(ratatui::style::Color::Black),
        );
        editor.editing_hex = false;
    }
}

pub fn save_theme_editor(state: &mut AppState) -> Result<()> {
    let Some(target) = theme_editor_save_target(state) else {
        return Ok(());
    };
    let local_path = local_theme_path(&state.project_root);
    let path = save_theme(
        &state.theme,
        &state.project_root,
        target,
        default_config_home().as_deref(),
    )?;
    let source = match target {
        ThemeSaveTarget::Local => ThemeSource::Local,
        ThemeSaveTarget::Global => ThemeSource::Global,
    };
    state.theme.source = source;
    state.theme_status = ThemeStatus::healthy(source, state.theme.version);
    if target == ThemeSaveTarget::Global && local_path.exists() {
        state.show_toast(
            ToastLevel::Warning,
            format!(
                "Saved global {}; local {} still overrides it",
                path.display(),
                local_path.display()
            ),
        );
    } else {
        state.show_toast(
            ToastLevel::Success,
            format!("Saved {} theme to {}", target.label(), path.display()),
        );
    }
    state.modal = None;
    Ok(())
}

fn theme_editor_save_target(state: &AppState) -> Option<ThemeSaveTarget> {
    match &state.modal {
        Some(Modal::ThemeEditor(editor)) => Some(editor.save_target),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::sorted_import_candidates;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRoot {
        path: PathBuf,
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    impl TempRoot {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "dd_dotstore_test_{prefix}_{}_{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp root");
            Self { path }
        }
    }

    #[test]
    fn import_candidates_are_sorted_newest_first() {
        let root = TempRoot::new("actions_sort");
        let old = root.path.join("a.json");
        let new = root.path.join("b.json");
        fs::write(&old, "{}").expect("write old");
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(&new, "{}").expect("write new");

        let read_dir = fs::read_dir(&root.path).expect("read dir");
        let files = sorted_import_candidates(read_dir);
        assert_eq!(files.first(), Some(&new));
        assert_eq!(files.get(1), Some(&old));
    }
}
