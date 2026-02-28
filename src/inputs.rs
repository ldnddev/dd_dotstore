use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions::{
    assign_destination, confirm_bulk, confirm_bulk_with_overwrite, export_to_path,
    import_from_path, open_export_picker, open_import_picker, selected_create_conflicts, undo_last,
};
use crate::state::{AppState, BrowserState, BulkAction, Modal, NodeKind};
use crate::tree::{flatten_visible, toggle_expand, toggle_selected};
use std::path::PathBuf;

pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    if state.modal.is_some() {
        return handle_modal_key(state, key);
    }

    match key.code {
        KeyCode::F(1) => {
            state.modal = Some(Modal::Help);
        }
        KeyCode::F(2) => {
            state.modal = Some(Modal::Credits);
        }
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),

        KeyCode::Down | KeyCode::Char('j') => move_selection(state, 1),
        KeyCode::Up | KeyCode::Char('k') => move_selection(state, -1),
        KeyCode::Char('G') => state
            .list_state
            .select(Some(state.nodes.len().saturating_sub(1))),
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.list_state.select(Some(0));
        }

        KeyCode::Char(' ') => toggle_selection(state),
        KeyCode::Enter => activate_selected(state),
        KeyCode::Char('e') => open_dest_browser(state),
        KeyCode::Right | KeyCode::Char('l') => set_folder_expanded(state, true),
        KeyCode::Left | KeyCode::Char('h') => set_folder_expanded(state, false),

        KeyCode::Char('s') if state.has_selected() => {
            state.modal = Some(Modal::ConfirmBulk {
                action: BulkAction::Create,
            });
        }
        KeyCode::Char('x') if state.has_selected() => {
            state.modal = Some(Modal::ConfirmBulk {
                action: BulkAction::Remove,
            });
        }

        KeyCode::Char('u') => undo_last(state)?,

        KeyCode::Char('/') => {
            state.modal = Some(Modal::Search);
            state.filter.clear();
        }

        KeyCode::Char('r') => {
            state.tree = crate::tree::build_tree(&state.project_root, &state.ignore_patterns);
            flatten_visible(state);
        }

        KeyCode::Char('I') => {
            state.modal = Some(Modal::IgnoreEditor { selected: 0 });
        }

        KeyCode::Char('i') => open_import_picker(state)?,
        KeyCode::Char('E') => open_export_picker(state)?,

        _ => {}
    }

    Ok(false)
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
    if let Some(idx) = state.list_state.selected()
        && idx < state.nodes.len()
    {
        let mut browser = BrowserState::new();
        browser.refresh_entries();
        state.modal = Some(Modal::EditDest {
            node_idx: idx,
            browser,
        });
    }
}

fn handle_modal_key(state: &mut AppState, key: KeyEvent) -> Result<bool> {
    let Some(current_modal) = state.modal.clone() else {
        return Ok(false);
    };

    match current_modal {
        Modal::ConfirmBulk { action } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if matches!(action, BulkAction::Create) {
                    let conflicts = selected_create_conflicts(state);
                    if !conflicts.is_empty() {
                        state.modal = Some(Modal::OverwriteWarning {
                            conflicts,
                            action_type: action,
                        });
                        return Ok(false);
                    }
                }
                confirm_bulk(state, action)?;
                state.modal = None;
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
                            if !matches!(state.modal, Some(Modal::Error { .. })) {
                                state.modal = None;
                            }
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
                        if !matches!(state.modal, Some(Modal::Error { .. })) {
                            state.modal = None;
                        }
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

        Modal::Search => {
            match key.code {
                KeyCode::Char(c) if c.is_ascii() => state.filter.push(c),
                KeyCode::Backspace => {
                    let _ = state.filter.pop();
                }
                KeyCode::Esc | KeyCode::Enter => state.modal = None,
                _ => {}
            }
            flatten_visible(state);
        }

        Modal::IgnoreEditor { mut selected } => {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(state.ignore_patterns.len().saturating_sub(1));
                }
                KeyCode::Delete | KeyCode::Char('d') => {
                    if selected < state.ignore_patterns.len() {
                        state.ignore_patterns.remove(selected);
                        selected = selected.min(state.ignore_patterns.len().saturating_sub(1));
                    }
                }
                KeyCode::Esc => {
                    state.modal = None;
                    return Ok(false);
                }
                _ => {}
            }
            state.modal = Some(Modal::IgnoreEditor { selected });
        }

        Modal::OverwriteWarning { action_type, .. } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                confirm_bulk_with_overwrite(state, action_type, true)?;
                state.modal = None;
            }
            _ => state.modal = None,
        },

        Modal::Error { .. } => {
            state.modal = None;
        }
        Modal::Help => match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::F(1) | KeyCode::Char('q') => {
                state.modal = None;
            }
            _ => {}
        },
        Modal::Credits => match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::F(2) | KeyCode::Char('q') => {
                state.modal = None;
            }
            _ => {}
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
                    if let Some(path) = files.get(selected) {
                        if let Err(err) = import_from_path(state, path) {
                            state.modal = Some(Modal::Error {
                                msg: format!("Import failed: {err}"),
                            });
                        }
                    } else {
                        state.modal = None;
                    }
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
                    export_to_path(state, &path)?;
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
    }

    Ok(false)
}

fn selected_rel_path(state: &AppState, node_idx: usize) -> Option<PathBuf> {
    state.nodes.get(node_idx).map(|node| node.path.clone())
}
