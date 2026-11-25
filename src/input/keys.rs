use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions::{
    action_targets, apply_adopt_candidates, apply_group_name, apply_live_theme_editor,
    apply_live_theme_picker, assign_destination, collect_preview_lines, confirm_bulk,
    confirm_bulk_with_overwrite, export_to_path, import_from_path, jump_to_doctor_source,
    open_about, open_adopt_picker, open_doctor, open_export_picker, open_group_editor,
    open_import_picker, save_theme_editor, select_group_of_highlight, selected_create_conflicts,
    step_back_theme_editor, toggle_settings, undo_last,
};
use crate::domain::{AppState, BrowserState, BulkAction, Modal, Node, NodeKind};
use crate::theme::{EditorKey, EditorOutcome, PickerOutcome, ThemeEditor};
use crate::tree::{
    AssignmentSource, find_mut_node, flatten_visible, rebuild_tree, toggle_expand, toggle_selected,
};
use crate::ui::settings::{NAV, PAGE_ABOUT, PAGE_HELP, PAGE_LAST, PAGE_THEMES};
use crate::ui::toast::ToastLevel;
use std::path::PathBuf;

fn is_quit_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Bracketed paste into the field that would take typed characters.
/// One call. Controls other than space are dropped. List hotkeys are left alone.
pub fn paste_text(state: &mut AppState, raw: &str) -> Result<()> {
    let graphic: String = raw
        .chars()
        .filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
        .collect();
    if graphic.is_empty() {
        return Ok(());
    }
    if state.modal.is_none() && state.show_settings && editing_theme_hex(state) {
        for ch in graphic.chars() {
            handle_key(state, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))?;
        }
        return Ok(());
    }
    if state.filter_editing && state.modal.is_none() && !state.show_settings {
        state.filter.push_str(&graphic);
        flatten_visible(state);
        return Ok(());
    }
    match state.modal.clone() {
        Some(Modal::EditDest {
            node_idx,
            mut browser,
        }) => {
            let filter: String = graphic.chars().filter(|ch| ch.is_ascii_graphic()).collect();
            browser.filter.push_str(&filter);
            browser.selected = 0;
            browser.clamp_selected();
            state.modal = Some(Modal::EditDest { node_idx, browser });
        }
        Some(Modal::IgnoreEditor {
            selected,
            mut draft,
        }) => {
            draft.push_str(&graphic);
            state.modal = Some(Modal::IgnoreEditor { selected, draft });
        }
        Some(Modal::ExportPicker { dir, mut filename }) => {
            for ch in graphic.chars() {
                if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                    filename.push(ch);
                }
            }
            state.modal = Some(Modal::ExportPicker { dir, filename });
        }
        Some(Modal::GroupEditor { paths, mut draft }) => {
            draft.push_str(&graphic);
            state.modal = Some(Modal::GroupEditor { paths, draft });
        }
        _ => {}
    }
    Ok(())
}

pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    if is_quit_key(&key) {
        return Ok(true);
    }

    if state.modal.is_some() {
        if matches!(key.code, KeyCode::F(1) | KeyCode::F(2)) {
            toggle_settings(state);
            return Ok(false);
        }
        if key.code == KeyCode::F(3) {
            open_about(state);
            return Ok(false);
        }
        return handle_modal_key(state, key);
    }

    if state.show_settings {
        handle_settings_key(state, key)?;
        return Ok(false);
    }

    if state.filter_editing {
        return handle_filter_key(state, key);
    }

    match key.code {
        KeyCode::F(1) | KeyCode::F(2) => toggle_settings(state),
        KeyCode::F(3) => open_about(state),
        KeyCode::Esc => {
            handle_esc_main(state);
            return Ok(false);
        }

        KeyCode::Down | KeyCode::Char('j') => move_selection(state, 1),
        KeyCode::Up | KeyCode::Char('k') => move_selection(state, -1),
        KeyCode::Char('G') => state
            .list_state
            .select(Some(state.nodes.len().saturating_sub(1))),
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.list_state.select(Some(0));
        }

        KeyCode::Char(' ') => toggle_selection(state),
        KeyCode::Char('m') => toggle_mode_for_highlighted(state),
        KeyCode::Char('M') => toggle_mode_for_selected(state),
        KeyCode::Enter => activate_selected(state),
        KeyCode::Char('e') => open_dest_browser(state),
        KeyCode::Right | KeyCode::Char('l') => set_folder_expanded(state, true),
        KeyCode::Left | KeyCode::Char('h') => set_folder_expanded(state, false),

        KeyCode::Char('s') | KeyCode::Char('p') if !action_targets(state).is_empty() => {
            state.modal = Some(Modal::Plan {
                action: BulkAction::Create,
                scroll: 0,
            });
        }
        KeyCode::Char('x') if !action_targets(state).is_empty() => {
            state.modal = Some(Modal::Plan {
                action: BulkAction::Remove,
                scroll: 0,
            });
        }

        KeyCode::Char('u') => undo_last(state)?,

        KeyCode::Char('/') => {
            state.filter_snapshot = state.filter.clone();
            state.filter_editing = true;
        }

        KeyCode::Char('r') => {
            rebuild_tree(state, AssignmentSource::LiveTree)?;
        }

        KeyCode::Char('I') => {
            state.modal = Some(Modal::IgnoreEditor {
                selected: 0,
                draft: String::new(),
            });
        }

        KeyCode::Char('i') => open_import_picker(state)?,
        KeyCode::Char('E') => open_export_picker(state)?,
        KeyCode::Char('t') => open_group_editor(state),
        KeyCode::Char('T') => select_group_of_highlight(state),
        KeyCode::Char('A') => open_adopt_picker(state),
        KeyCode::Char('D') => open_doctor(state),

        _ => {}
    }

    Ok(false)
}

/// Closed match. Never quits. Flatten after every filter mutation, including Esc restore.
fn handle_filter_key(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Esc => {
            state.filter = state.filter_snapshot.clone();
            state.filter_editing = false;
            flatten_visible(state);
        }
        KeyCode::Enter => {
            state.filter_editing = false;
            flatten_visible(state);
        }
        KeyCode::Backspace => {
            let _ = state.filter.pop();
            flatten_visible(state);
        }
        KeyCode::Char(c) if key.modifiers.is_empty() && (c.is_ascii_graphic() || c == ' ') => {
            // Same gate as dest browser. Graphic chars including q, /, j, k, s.
            // Space inserts. Ctrl+g is Char('g')+CONTROL and must be swallowed.
            state.filter.push(c);
            flatten_visible(state);
        }
        _ => {
            // Swallow F1/F2, arrows, Ctrl+g / Alt+…, etc.
        }
    }
    Ok(false)
}

fn handle_esc_main(state: &mut AppState) {
    if !state.filter.is_empty() {
        state.filter.clear();
        flatten_visible(state);
        return;
    }
    if any_tree_selected(&state.tree) {
        clear_tree_selected(&mut state.tree);
        flatten_visible(state);
    }
}

fn any_tree_selected(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| {
        n.selected
            || match &n.kind {
                NodeKind::Folder { children, .. } => any_tree_selected(children),
                NodeKind::File { .. } => false,
            }
    })
}

fn clear_tree_selected(nodes: &mut [Node]) {
    for n in nodes {
        n.selected = false;
        if let NodeKind::Folder { children, .. } = &mut n.kind {
            clear_tree_selected(children);
        }
    }
}

fn move_selection(state: &mut AppState, delta: isize) {
    if state.nodes.is_empty() {
        return;
    }
    let cur = state.list_state.selected().unwrap_or(0) as isize;
    let next = (cur + delta).clamp(0, state.nodes.len() as isize - 1);
    state.list_state.select(Some(next as usize));
}

fn toggle_selection(state: &mut AppState) {
    let Some(idx) = state.list_state.selected() else {
        return;
    };
    if idx >= state.nodes.len() {
        return;
    }

    let selected = state.nodes[idx].clone();
    toggle_selected(state, &selected.path);

    flatten_visible(state);
    state
        .list_state
        .select(Some(idx.min(state.nodes.len().saturating_sub(1))));
}

fn toggle_mode_for_highlighted(state: &mut AppState) {
    let Some(idx) = state.list_state.selected() else {
        return;
    };
    let Some(selected) = state.nodes.get(idx).cloned() else {
        return;
    };

    if let Some(node) = find_mut_node(&mut state.tree, &selected.path) {
        node.action_mode = node.action_mode.toggled();
        state.mark_dirty();
    }

    flatten_visible(state);
    state
        .list_state
        .select(Some(idx.min(state.nodes.len().saturating_sub(1))));
}

fn toggle_mode_for_selected(state: &mut AppState) {
    let selected_paths: Vec<_> = state
        .nodes
        .iter()
        .filter(|node| node.selected)
        .map(|node| node.path.clone())
        .collect();

    if selected_paths.is_empty() {
        toggle_mode_for_highlighted(state);
        return;
    }

    let target_mode = selected_paths
        .first()
        .and_then(|path| find_mut_node(&mut state.tree, path))
        .map(|node| node.action_mode.toggled());

    if let Some(target_mode) = target_mode {
        let mut changed = false;
        for path in selected_paths {
            if let Some(node) = find_mut_node(&mut state.tree, &path)
                && node.action_mode != target_mode
            {
                node.action_mode = target_mode;
                changed = true;
            }
        }
        if changed {
            state.mark_dirty();
        }
    }

    flatten_visible(state);
}

fn activate_selected(state: &mut AppState) {
    open_dest_browser(state);
}

fn set_folder_expanded(state: &mut AppState, expand: bool) {
    let Some(idx) = state.list_state.selected() else {
        return;
    };
    if idx >= state.nodes.len() {
        return;
    }

    let selected = state.nodes[idx].clone();
    if let NodeKind::Folder { expanded, .. } = selected.kind
        && expanded != expand
    {
        toggle_expand(state, &selected.path);
        flatten_visible(state);
        state
            .list_state
            .select(Some(idx.min(state.nodes.len().saturating_sub(1))));
    }
}

fn open_dest_browser(state: &mut AppState) {
    if let Some(idx) = state.list_state.selected() {
        open_dest_browser_for_idx(state, idx);
    }
}

pub(crate) fn open_dest_browser_for_idx(state: &mut AppState, idx: usize) {
    if idx >= state.nodes.len() {
        return;
    }
    state.list_state.select(Some(idx));
    let mut browser = BrowserState::new();
    browser.refresh_entries();
    state.modal = Some(Modal::EditDest {
        node_idx: idx,
        browser,
    });
}

fn remove_ignore_at(state: &mut AppState, selected: &mut usize) -> Result<()> {
    if *selected < state.ignore_patterns.len() {
        state.ignore_patterns.remove(*selected);
        if !state.ignore_patterns.is_empty() {
            *selected = (*selected).min(state.ignore_patterns.len().saturating_sub(1));
        } else {
            *selected = 0;
        }
        state.mark_dirty();
        rebuild_tree(state, AssignmentSource::LiveTree)?;
        state.persist_now_or_toast();
    }
    Ok(())
}

fn handle_modal_key(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    let Some(current_modal) = state.modal.clone() else {
        return Ok(false);
    };

    match current_modal {
        Modal::Plan { action, scroll } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if matches!(action, BulkAction::Create) {
                    let conflicts = selected_create_conflicts(state);
                    if !conflicts.is_empty() {
                        state.modal = Some(Modal::OverwriteWarning {
                            conflicts,
                            action_type: action,
                            scroll: 0,
                        });
                        return Ok(false);
                    }
                }
                confirm_bulk(state, action)?;
                state.modal = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max_scroll = collect_preview_lines(state, action).len().saturating_sub(1);
                state.modal = Some(Modal::Plan {
                    action,
                    scroll: (scroll + 1).min(max_scroll),
                });
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.modal = Some(Modal::Plan {
                    action,
                    scroll: scroll.saturating_sub(1),
                });
            }
            _ => state.modal = None,
        },

        Modal::EditDest {
            node_idx,
            mut browser,
        } => {
            let filtered = browser.filtered_indices();
            match key.code {
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    browser.selected = browser.selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    browser.selected = (browser.selected + 1).min(filtered.len().saturating_sub(1));
                }
                KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                    if !browser.filter.is_empty() {
                        let _ = browser.filter.pop();
                        browser.selected = 0;
                    } else if browser.current != browser.root
                        && let Some(parent) = browser.current.parent()
                    {
                        browser.current = parent.to_path_buf();
                        browser.selected = 0;
                        browser.refresh_entries();
                    }
                }
                KeyCode::Char('~') => {
                    browser.current = browser.root.clone();
                    browser.selected = 0;
                    browser.refresh_entries();
                }
                KeyCode::Char('g') if key.modifiers.is_empty() && browser.filter.is_empty() => {
                    browser.selected = 0;
                }
                KeyCode::Char('G') if key.modifiers.is_empty() && browser.filter.is_empty() => {
                    browser.selected = filtered.len().saturating_sub(1);
                }
                KeyCode::PageUp => {
                    browser.selected = browser.selected.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    browser.selected =
                        (browser.selected + 10).min(filtered.len().saturating_sub(1));
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    if let Some(entry_idx) = filtered.get(browser.selected)
                        && let Some(entry) = browser.entries.get(*entry_idx)
                    {
                        if entry.is_dir {
                            if entry.name == ".." {
                                if let Some(parent) = browser.current.parent() {
                                    browser.current = parent.to_path_buf();
                                }
                            } else {
                                browser.current = browser.current.join(&entry.name);
                            }
                            browser.selected = 0;
                            browser.refresh_entries();
                        } else if let Some(rel_src) = selected_rel_path(state, node_idx) {
                            let dest = browser.current.join(&entry.name);
                            assign_destination(state, &rel_src, dest)?;
                            state.modal = None;
                            return Ok(false);
                        }
                    }
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(rel_src) = selected_rel_path(state, node_idx) {
                        let src_name = rel_src
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "dotfile".to_string());
                        let dest = browser.current.join(src_name);
                        assign_destination(state, &rel_src, dest)?;
                        state.modal = None;
                        return Ok(false);
                    }
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    browser.filter.clear();
                    browser.selected = 0;
                }
                KeyCode::Char(c) if key.modifiers.is_empty() && c.is_ascii_graphic() => {
                    browser.filter.push(c);
                    browser.selected = 0;
                }
                _ => {}
            }
            browser.clamp_selected();
            if state.modal.is_some() {
                state.modal = Some(Modal::EditDest { node_idx, browser });
            }
        }

        Modal::IgnoreEditor {
            mut selected,
            mut draft,
        } => {
            match key.code {
                KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if !state.ignore_patterns.is_empty() {
                        selected =
                            (selected + 1).min(state.ignore_patterns.len().saturating_sub(1));
                    }
                }
                KeyCode::Char('k') if key.modifiers.is_empty() && draft.is_empty() => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Char('j') if key.modifiers.is_empty() && draft.is_empty() => {
                    if !state.ignore_patterns.is_empty() {
                        selected =
                            (selected + 1).min(state.ignore_patterns.len().saturating_sub(1));
                    }
                }
                KeyCode::Delete => {
                    remove_ignore_at(state, &mut selected)?;
                }
                KeyCode::Char('d') if key.modifiers.is_empty() && draft.is_empty() => {
                    remove_ignore_at(state, &mut selected)?;
                }
                KeyCode::Enter => {
                    if !draft.is_empty() {
                        state.ignore_patterns.push(std::mem::take(&mut draft));
                        selected = state.ignore_patterns.len().saturating_sub(1);
                        state.mark_dirty();
                        rebuild_tree(state, AssignmentSource::LiveTree)?;
                        state.persist_now_or_toast();
                    }
                }
                KeyCode::Backspace => {
                    let _ = draft.pop();
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() && (c.is_ascii_graphic() || c == ' ') =>
                {
                    draft.push(c);
                }
                _ => {}
            }
            if state.modal.is_some() {
                state.modal = Some(Modal::IgnoreEditor { selected, draft });
            }
        }

        Modal::OverwriteWarning {
            action_type,
            conflicts,
            scroll,
        } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                confirm_bulk_with_overwrite(state, action_type, true)?;
                state.modal = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max_scroll = conflicts.len().saturating_sub(1);
                state.modal = Some(Modal::OverwriteWarning {
                    conflicts,
                    action_type,
                    scroll: (scroll + 1).min(max_scroll),
                });
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.modal = Some(Modal::OverwriteWarning {
                    conflicts,
                    action_type,
                    scroll: scroll.saturating_sub(1),
                });
            }
            _ => state.modal = None,
        },

        Modal::ImportPicker {
            files,
            mut selected,
        } => {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(files.len().saturating_sub(1));
                }
                KeyCode::Enter => {
                    if let Some(path) = files.get(selected)
                        && let Err(err) = import_from_path(state, path)
                    {
                        state.show_toast(ToastLevel::Error, format!("Import failed: {err}"));
                    }
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                _ => {}
            }
            state.modal = Some(Modal::ImportPicker { files, selected });
        }

        Modal::ExportPicker { dir, mut filename } => {
            match key.code {
                KeyCode::Char(c) if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') => {
                    filename.push(c);
                }
                KeyCode::Backspace => {
                    let _ = filename.pop();
                }
                KeyCode::Enter => {
                    let name = if filename.trim().is_empty() {
                        "dd_dotstore_export.json".to_string()
                    } else if filename.ends_with(".json") {
                        filename.clone()
                    } else {
                        format!("{filename}.json")
                    };
                    let path = dir.join(name);
                    if let Err(err) = export_to_path(state, &path) {
                        state.show_toast(ToastLevel::Error, format!("Export failed: {err}"));
                    }
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                _ => {}
            }
            state.modal = Some(Modal::ExportPicker { dir, filename });
        }

        Modal::GroupEditor { paths, mut draft } => {
            match key.code {
                KeyCode::Char(c)
                    if key.modifiers.is_empty() && (c.is_ascii_graphic() || c == ' ') =>
                {
                    draft.push(c);
                }
                KeyCode::Backspace => {
                    let _ = draft.pop();
                }
                KeyCode::Enter => {
                    apply_group_name(state, &paths, &draft);
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                _ => {}
            }
            if state.modal.is_some() {
                state.modal = Some(Modal::GroupEditor { paths, draft });
            }
        }

        Modal::Adopt {
            candidates,
            mut selected,
            mut checked,
        } => {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(candidates.len().saturating_sub(1));
                }
                KeyCode::Char(' ') => {
                    if let Some(flag) = checked.get_mut(selected) {
                        *flag = !*flag;
                    }
                }
                KeyCode::Char('a') => {
                    let all = checked.iter().all(|c| *c);
                    for flag in checked.iter_mut() {
                        *flag = !all;
                    }
                }
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let chosen: Vec<_> = if checked.iter().any(|c| *c) {
                        candidates
                            .iter()
                            .zip(checked.iter())
                            .filter(|(_, on)| **on)
                            .map(|(c, _)| c.clone())
                            .collect()
                    } else {
                        candidates.get(selected).cloned().into_iter().collect()
                    };
                    apply_adopt_candidates(state, &chosen);
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                _ => {}
            }
            if state.modal.is_some() {
                state.modal = Some(Modal::Adopt {
                    candidates,
                    selected,
                    checked,
                });
            }
        }

        Modal::Doctor {
            findings,
            mut selected,
            mut scroll,
        } => {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                    if selected < scroll {
                        scroll = selected;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(findings.len().saturating_sub(1));
                    if selected > scroll + 8 {
                        scroll = selected.saturating_sub(8);
                    }
                }
                KeyCode::Enter => {
                    if let Some(src) = findings.get(selected).and_then(|f| f.src.clone()) {
                        jump_to_doctor_source(state, &src);
                    }
                    state.modal = None;
                    return Ok(false);
                }
                KeyCode::Char('A') => {
                    open_adopt_picker(state);
                    return Ok(false);
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                _ => {}
            }
            if state.modal.is_some() {
                state.modal = Some(Modal::Doctor {
                    findings,
                    selected,
                    scroll,
                });
            }
        }
    }

    Ok(false)
}

fn handle_settings_key(state: &mut AppState, key: KeyEvent) -> Result<()> {
    if matches!(key.code, KeyCode::F(1) | KeyCode::F(2)) {
        crate::actions::close_settings(state, true);
        return Ok(());
    }
    if key.code == KeyCode::F(3) {
        open_about(state);
        return Ok(());
    }
    if key.code == KeyCode::PageDown {
        scroll_settings(state, 10);
        return Ok(());
    }
    if key.code == KeyCode::PageUp {
        scroll_settings(state, -10);
        return Ok(());
    }
    if settings_section_key(state, key) {
        return Ok(());
    }
    if state.settings_nav == PAGE_HELP || state.settings_nav == PAGE_ABOUT {
        match key.code {
            KeyCode::Esc => crate::actions::close_settings(state, true),
            KeyCode::Down | KeyCode::Char('j') => scroll_settings(state, 1),
            KeyCode::Up | KeyCode::Char('k') => scroll_settings(state, -1),
            KeyCode::Home | KeyCode::Char('g') => state.help_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => state.help_scroll = state.help_scroll_max,
            _ => {}
        }
        return Ok(());
    }
    if state.settings_nav == PAGE_THEMES {
        if save_target_key(state, &key) {
            toggle_save_target(state);
        } else if let Some(ek) = map_editor_key(key) {
            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
            if state.theme_editor.is_some() {
                dispatch_theme_editor(state, ek, shift);
            } else {
                dispatch_theme_picker(state, ek);
            }
        }
        return Ok(());
    }
    if key.code == KeyCode::Esc {
        crate::actions::close_settings(state, true);
    }
    Ok(())
}

pub(crate) fn scroll_settings(state: &mut AppState, delta: i32) {
    state.settings_follow = false;
    if delta < 0 {
        state.help_scroll = state
            .help_scroll
            .saturating_sub(delta.unsigned_abs() as u16);
    } else {
        state.help_scroll = state
            .help_scroll
            .saturating_add(delta as u16)
            .min(state.help_scroll_max);
    }
}

pub(crate) fn settings_nav_click(state: &mut AppState, col: u16, row: u16) -> bool {
    let area = state.pointer.settings_nav_area;
    if area.width == 0 || area.height == 0 {
        return false;
    }
    let inside = col >= area.x
        && row >= area.y
        && col < area.x.saturating_add(area.width)
        && row < area.y.saturating_add(area.height);
    if !inside {
        return false;
    }
    let rel = usize::from(row.saturating_sub(area.y));
    if rel >= NAV.len() {
        return true;
    }
    state.settings_nav = rel as u8;
    state.help_scroll = 0;
    state.settings_follow = true;
    true
}

fn editing_theme_hex(state: &AppState) -> bool {
    state
        .theme_editor
        .as_ref()
        .is_some_and(|editor| editor.editing_hex)
}

fn settings_section_key(state: &mut AppState, key: KeyEvent) -> bool {
    let Some(dir) = settings_section_dir(key) else {
        return false;
    };
    let tab = matches!(key.code, KeyCode::Tab | KeyCode::BackTab);
    if !tab && editing_theme_hex(state) {
        return false;
    }
    let next = (state.settings_nav as isize + dir).clamp(0, PAGE_LAST as isize) as u8;
    if next != state.settings_nav {
        state.settings_nav = next;
        state.help_scroll = 0;
        state.settings_follow = true;
    }
    true
}

fn settings_section_dir(key: KeyEvent) -> Option<isize> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
    {
        return None;
    }
    match key.code {
        KeyCode::BackTab => Some(-1),
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some(-1)
            } else {
                Some(1)
            }
        }
        KeyCode::Char(']') => Some(1),
        KeyCode::Char('[') => Some(-1),
        _ => None,
    }
}

fn save_target_key(state: &AppState, key: &KeyEvent) -> bool {
    if editing_theme_hex(state) || key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }
    matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T'))
}

fn toggle_save_target(state: &mut AppState) {
    let from_editor = if let Some(editor) = state.theme_editor.as_mut() {
        editor.save_target = editor.save_target.toggled();
        Some(editor.save_target)
    } else {
        None
    };
    if let Some(target) = from_editor {
        if let Some(picker) = state.theme_picker.as_mut() {
            picker.save_target = target;
        }
        return;
    }
    if let Some(picker) = state.theme_picker.as_mut() {
        picker.save_target = picker.save_target.toggled();
    }
}

fn theme_motion(key: EditorKey) -> bool {
    matches!(
        key,
        EditorKey::Down
            | EditorKey::Up
            | EditorKey::Char('j')
            | EditorKey::Char('k')
            | EditorKey::Char('g')
            | EditorKey::Char('G')
    )
}

fn preview_theme_row(state: &mut AppState) {
    let outcome = {
        let Some(picker) = state.theme_picker.as_mut() else {
            return;
        };
        picker.preview_selected()
    };
    match outcome {
        PickerOutcome::Applied => apply_live_theme_picker(state),
        PickerOutcome::Error(msg) => state.show_toast(ToastLevel::Error, msg),
        _ => {}
    }
}

fn dispatch_theme_picker(state: &mut AppState, ek: EditorKey) {
    let outcome = {
        let Some(picker) = state.theme_picker.as_mut() else {
            return;
        };
        picker.handle(ek)
    };
    match outcome {
        PickerOutcome::Applied => apply_live_theme_picker(state),
        PickerOutcome::EditColors => {
            let Some(picker) = state.theme_picker.as_ref() else {
                return;
            };
            let mut editor =
                ThemeEditor::new(picker.palette.clone(), crate::theme::extra_theme_fields());
            editor.save_target = picker.save_target;
            state.theme_editor = Some(editor);
        }
        PickerOutcome::RequestSave => {
            if let Err(err) = save_theme_editor(state) {
                state.show_toast(ToastLevel::Error, format!("Save failed: {err}"));
            }
        }
        PickerOutcome::Error(msg) => state.show_toast(ToastLevel::Error, msg),
        PickerOutcome::Closed { .. } => crate::actions::close_settings(state, true),
        PickerOutcome::None => {
            if theme_motion(ek) {
                preview_theme_row(state);
                state.settings_follow = true;
            }
        }
    }
}

fn dispatch_theme_editor(state: &mut AppState, ek: EditorKey, shift: bool) {
    let outcome = {
        let Some(editor) = state.theme_editor.as_mut() else {
            return;
        };
        editor.handle(ek, shift)
    };
    match outcome {
        EditorOutcome::PaletteChanged => apply_live_theme_editor(state),
        EditorOutcome::RequestSave => {
            if let Err(err) = save_theme_editor(state) {
                state.show_toast(ToastLevel::Error, format!("Save failed: {err}"));
            }
        }
        EditorOutcome::Closed { .. } => step_back_theme_editor(state),
        EditorOutcome::HexError(err) => {
            state.show_toast(ToastLevel::Error, format!("Invalid hex: {err}"));
        }
        EditorOutcome::None => {
            let editing = state
                .theme_editor
                .as_ref()
                .is_some_and(|editor| editor.editing_hex);
            if theme_motion(ek) && !editing {
                state.settings_follow = true;
            }
        }
    }
}

fn map_editor_key(key: KeyEvent) -> Option<EditorKey> {
    Some(match key.code {
        KeyCode::Up => EditorKey::Up,
        KeyCode::Down => EditorKey::Down,
        KeyCode::Left => EditorKey::Left,
        KeyCode::Right => EditorKey::Right,
        KeyCode::Tab => EditorKey::Tab,
        KeyCode::Enter => EditorKey::Enter,
        KeyCode::Esc => EditorKey::Esc,
        KeyCode::Backspace => EditorKey::Backspace,
        KeyCode::Char(c) => EditorKey::Char(c),
        _ => return None,
    })
}

pub(crate) fn selected_rel_path(state: &AppState, node_idx: usize) -> Option<PathBuf> {
    state.nodes.get(node_idx).map(|node| node.path.clone())
}
