use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

use crate::actions::{
    assign_destination, confirm_bulk, confirm_bulk_with_overwrite, export_to_path,
    import_from_path, open_export_picker, open_import_picker, selected_create_conflicts, undo_last,
};
use crate::state::{AppState, BrowserState, BulkAction, Modal, Node, NodeKind};
use crate::toast::ToastLevel;
use crate::tree::{
    AssignmentSource, find_mut_node, flatten_visible, rebuild_tree, toggle_expand, toggle_selected,
};
use ratatui::layout::Rect;
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
        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
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

        // Preview / dry-run safety feature: 'p' shows detailed plan of what Create would do
        // (no changes until you confirm with Y). Use 's'/'x' for direct, or 'p' then Y.
        // For remove preview, trigger 'x' and review the (enhanced) confirm.
        KeyCode::Char('p') if state.has_selected() => {
            state.modal = Some(Modal::PreviewBulk {
                action: BulkAction::Create,
            });
        }

        KeyCode::Char('u') => undo_last(state)?,

        KeyCode::Char('/') => {
            state.modal = Some(Modal::Search);
            state.filter.clear();
        }

        KeyCode::Char('r') => {
            rebuild_tree(state, AssignmentSource::LiveTree)?;
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
                            scroll: 0,
                        });
                        return Ok(false);
                    }
                }
                confirm_bulk(state, action)?;
                state.modal = None;
            }
            _ => state.modal = None,
        },

        Modal::PreviewBulk { action } => match key.code {
            // From preview (dry-run safety), y proceeds to actual apply (same flow as confirm)
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
    }

    Ok(false)
}

fn selected_rel_path(state: &AppState, node_idx: usize) -> Option<PathBuf> {
    state.nodes.get(node_idx).map(|node| node.path.clone())
}

// =============================================================================
// Mouse support
// - Wheel only over source panel (per spec)
// - Drag/click on scrollbar (right edge of source or browser modal) for scrolling the view
// - Far left zone (~ checkbox) toggles selection like Space
// - Glyph zone on folders toggles expand/collapse
// - Click sets highlight cursor
// - Double-click (on name part) activates (edit dest) for main list
// - For EditDest browser: click moves highlight, double-click on name acts (descend or assign), scrollbar drag scrolls
// - Outside any open modal rect cancels the modal (like Esc)
// - ImportPicker: click chooses row (use Enter to confirm, per spec)
// - Toast click dismisses
// =============================================================================

pub fn handle_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    // Clear transient drag on button up regardless
    if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
        state.scrollbar_dragging = false;
    }

    // Toast click-to-dismiss (any button)
    if let Some(tarea) = state.toast_area
        && rect_contains(tarea, mouse.column, mouse.row)
    {
        state.toast = None;
        state.toast_area = None;
        return Ok(false);
    }

    if state.modal.is_some() {
        return handle_modal_mouse(state, mouse);
    }

    handle_main_mouse(state, mouse)
}

fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x && col < area.x + area.width && row >= area.y && row < area.y + area.height
}

fn is_over_scrollbar(area: &Rect, col: u16, row: u16) -> bool {
    if area.width < 2 {
        return false;
    }
    let sb_col = area.x + area.width - 1;
    col == sb_col && row >= area.y && row < area.y + area.height
}

fn handle_main_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let delta: usize = if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                8
            } else {
                3
            };
            if rect_contains(state.source_area, mouse.column, mouse.row) {
                // Wheel on source
                if state.nodes.is_empty() {
                    return Ok(false);
                }
                let cur_off = state.list_state.offset();
                let new_off = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    cur_off.saturating_sub(delta)
                } else {
                    (cur_off + delta).min(state.nodes.len().saturating_sub(1))
                };
                *state.list_state.offset_mut() = new_off;

                // Snap highlight cursor into the new viewport
                let vh = state.source_area.height.saturating_sub(2) as usize;
                let end = new_off + vh.saturating_sub(1);
                if let Some(sel) = state.list_state.selected() {
                    if sel < new_off || sel > end {
                        state
                            .list_state
                            .select(Some(new_off.min(state.nodes.len().saturating_sub(1))));
                    }
                } else {
                    state
                        .list_state
                        .select(Some(new_off.min(state.nodes.len().saturating_sub(1))));
                }
            } else if rect_contains(state.status_area, mouse.column, mouse.row) {
                // Wheel over Destinations panel: scroll the right panel for information density
                let total = state.destination_paths.len();
                if total == 0 {
                    return Ok(false);
                }
                let cur_off = state.status_list_state.offset();
                let new_off = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    cur_off.saturating_sub(delta)
                } else {
                    (cur_off + delta).min(total.saturating_sub(1))
                };
                *state.status_list_state.offset_mut() = new_off;
            } else {
                return Ok(false);
            }
        }

        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
            if !rect_contains(state.source_area, mouse.column, mouse.row)
                && !rect_contains(state.status_area, mouse.column, mouse.row)
            {
                return Ok(false);
            }

            // Scrollbar drag/click takes precedence (for both panels)
            let over_source_sb = is_over_scrollbar(&state.source_area, mouse.column, mouse.row);
            let over_status_sb = is_over_scrollbar(&state.status_area, mouse.column, mouse.row);
            if over_source_sb || over_status_sb || state.scrollbar_dragging {
                state.scrollbar_dragging = true;
                // During active drag, decide panel by which rect the mouse row is in (allows dragging off the thin sb column)
                if rect_contains(state.status_area, mouse.column, mouse.row) {
                    update_status_scrollbar(state, mouse.row);
                } else if rect_contains(state.source_area, mouse.column, mouse.row) {
                    update_source_scrollbar(state, mouse.row);
                } else if state.status_area.height > 0 {
                    // Fallback: if mouse y roughly in status vertical range
                    let in_status_y = mouse.row >= state.status_area.y
                        && mouse.row < state.status_area.y + state.status_area.height;
                    if in_status_y {
                        update_status_scrollbar(state, mouse.row);
                    } else {
                        update_source_scrollbar(state, mouse.row);
                    }
                } else {
                    update_source_scrollbar(state, mouse.row);
                }
                return Ok(false);
            }

            if rect_contains(state.source_area, mouse.column, mouse.row)
                && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            {
                // Double-click detection (only counts on name part later)
                let now = Instant::now();
                let is_double = if let Some((lx, ly, lt)) = state.last_mouse_click_pos {
                    lx == mouse.column
                        && ly == mouse.row
                        && now.duration_since(lt).as_millis() < 420
                } else {
                    false
                };
                state.last_mouse_click_pos = Some((mouse.column, mouse.row, now));

                let (maybe_idx, is_name_part) = hit_test_source_row(state, mouse.column, mouse.row);
                if let Some(idx) = maybe_idx {
                    // Capture highlight before moving so Shift+click range is not a one-row select.
                    let prev = state.list_state.selected();
                    state.list_state.select(Some(idx));

                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        if let Some(cur) = prev {
                            let start = cur.min(idx);
                            let end = cur.max(idx);
                            for i in start..=end {
                                if let Some(n) = state.nodes.get(i) {
                                    let p = n.path.clone();
                                    if let Some(node) = find_mut_node(&mut state.tree, &p) {
                                        node.selected = true;
                                    }
                                }
                            }
                            flatten_visible(state);
                            // Re-clamp the highlight (flatten rarely changes indices for pure select-range on visible items)
                            state
                                .list_state
                                .select(Some(idx.min(state.nodes.len().saturating_sub(1))));
                        }
                        // Skip other zone actions for shift range
                    } else {
                        let rel_x = compute_rel_x(state, mouse.column);
                        let is_checkbox = rel_x < 4;

                        if is_checkbox {
                            toggle_selection_at(state, idx);
                        } else if is_folder_glyph(state, idx, rel_x) {
                            toggle_expand_at(state, idx);
                        } else if is_double && is_name_part {
                            // Double click only on the name (per spec)
                            open_dest_browser_for(state, idx);
                        }
                        // Single click on name/glyph area: just the select we did above. Good.
                    }
                }
                // (no else for drag comment here, as drag is handled before this if)
            } else if rect_contains(state.status_area, mouse.column, mouse.row) {
                // Click in Destinations panel: jump highlight to the corresponding source item (polish + navigation)
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                    let content_top = state.status_area.y + 1;
                    if mouse.row >= content_top {
                        let rel = (mouse.row - content_top) as usize;
                        let offset = state.status_list_state.offset();
                        let global = offset + rel;
                        if global < state.destination_paths.len() {
                            let target_path = state.destination_paths[global].clone();
                            // Auto-expand ancestors so the item becomes visible in the source tree
                            crate::tree::expand_ancestors(state, &target_path);
                            flatten_visible(state);
                            // Re-find after possible expansion (indices may shift)
                            if let Some(idx) =
                                state.nodes.iter().position(|n| n.path == target_path)
                            {
                                state.list_state.select(Some(idx));
                            }
                        }
                    }
                }
            }
        }

        MouseEventKind::Up(MouseButton::Left) => {
            state.scrollbar_dragging = false;
        }

        _ => {}
    }
    Ok(false)
}

fn update_source_scrollbar(state: &mut AppState, mouse_row: u16) {
    let area = state.source_area;
    if area.height < 3 || state.nodes.is_empty() {
        return;
    }
    let track_top = area.y + 1;
    let track_height = area.height.saturating_sub(2);
    let total = state.nodes.len();
    let new_offset = if track_height == 0 {
        0
    } else {
        let rel = mouse_row.saturating_sub(track_top) as usize;
        let pos = (rel as f64 * total as f64 / track_height as f64) as usize;
        pos.min(total.saturating_sub(1))
    };
    *state.list_state.offset_mut() = new_offset;

    // Keep highlight visible in the scrolled viewport
    let vh = track_height as usize;
    let end = new_offset + vh.saturating_sub(1);
    if let Some(sel) = state.list_state.selected() {
        if sel < new_offset || sel > end {
            state
                .list_state
                .select(Some(new_offset.min(state.nodes.len().saturating_sub(1))));
        }
    } else {
        state
            .list_state
            .select(Some(new_offset.min(state.nodes.len().saturating_sub(1))));
    }
}

fn update_status_scrollbar(state: &mut AppState, mouse_row: u16) {
    let area = state.status_area;
    let total = state.destination_paths.len();
    if area.height < 3 || total == 0 {
        return;
    }
    let track_top = area.y + 1;
    let track_height = area.height.saturating_sub(2);
    let new_offset = if track_height == 0 {
        0
    } else {
        let rel = mouse_row.saturating_sub(track_top) as usize;
        let pos = (rel as f64 * total as f64 / track_height as f64) as usize;
        pos.min(total.saturating_sub(1))
    };
    *state.status_list_state.offset_mut() = new_offset;
}

fn hit_test_source_row(state: &AppState, col: u16, row: u16) -> (Option<usize>, bool) {
    if !rect_contains(state.source_area, col, row) || state.nodes.is_empty() {
        return (None, false);
    }
    let inner_x = state.source_area.x + 1;
    let inner_y = state.source_area.y + 1;
    if row < inner_y {
        return (None, false);
    }
    let rel_y = row - inner_y;
    let rel_x = col.saturating_sub(inner_x);
    let offset = state.list_state.offset();
    let idx = offset + rel_y as usize;
    if idx >= state.nodes.len() {
        return (None, false);
    }
    // Name part starts after the fixed prefix (~13) + tree connector (~4 chars)
    let is_name_part = rel_x >= 17;
    (Some(idx), is_name_part)
}

fn compute_rel_x(state: &AppState, col: u16) -> u16 {
    let inner_x = state.source_area.x + 1;
    col.saturating_sub(inner_x)
}

fn is_folder_glyph(state: &AppState, idx: usize, rel_x: u16) -> bool {
    let Some(node) = state.nodes.get(idx) else {
        return false;
    };
    if !matches!(node.kind, NodeKind::Folder { .. }) {
        return false;
    }
    // Click the tree connector / structure area (after fixed 13 char prefix) to toggle expand/collapse.
    // Covers the tree chars (e.g. "├─ ", "└─ ") for folders. Now with improved tree visuals.
    (12..=18).contains(&rel_x)
}

fn toggle_selection_at(state: &mut AppState, idx: usize) {
    if idx >= state.nodes.len() {
        return;
    }
    let path = state.nodes[idx].path.clone();
    toggle_selected(state, &path);
    flatten_visible(state);
    state
        .list_state
        .select(Some(idx.min(state.nodes.len().saturating_sub(1))));
}

fn toggle_expand_at(state: &mut AppState, idx: usize) {
    if idx >= state.nodes.len() {
        return;
    }
    let node = state.nodes[idx].clone();
    if let NodeKind::Folder { .. } = &node.kind {
        // toggle to opposite
        let path = node.path.clone();
        toggle_expand(state, &path);
        flatten_visible(state);
        state
            .list_state
            .select(Some(idx.min(state.nodes.len().saturating_sub(1))));
    }
}

fn open_dest_browser_for(state: &mut AppState, idx: usize) {
    if idx >= state.nodes.len() {
        return;
    }
    state.list_state.select(Some(idx));
    // Reuse the existing logic that reads the (now updated) selected
    if let Some(i) = state.list_state.selected()
        && i < state.nodes.len()
    {
        let mut browser = BrowserState::new();
        browser.refresh_entries();
        state.modal = Some(Modal::EditDest {
            node_idx: i,
            browser,
        });
    }
}

fn handle_modal_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    let Some(modal_area) = state.current_modal_area else {
        // No area yet (shouldn't happen), fall back to cancel on any click
        state.modal = None;
        return Ok(false);
    };

    let inside_modal = rect_contains(modal_area, mouse.column, mouse.row);
    if !inside_modal {
        // Outside click cancels the modal (Esc semantics for most)
        state.modal = None;
        return Ok(false);
    }

    // Dispatch by modal type
    // We clone like the key handler does for mutation of inner state
    let current_modal = state.modal.clone();
    match current_modal {
        Some(Modal::EditDest {
            node_idx,
            mut browser,
        }) => {
            let filtered = browser.filtered_indices();
            let total_f = filtered.len();

            // Scrollbar drag/click for the browser list (right edge of modal)
            let sb_col = modal_area.x + modal_area.width.saturating_sub(1);
            let over_sb = mouse.column == sb_col;
            if matches!(
                mouse.kind,
                MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left)
            ) && over_sb
            {
                if total_f > 0 {
                    let track_top = modal_area.y + 1;
                    let track_h = modal_area.height.saturating_sub(2);
                    let rel = mouse.row.saturating_sub(track_top) as usize;
                    let pos = if track_h > 0 {
                        (rel as f64 * total_f as f64 / track_h as f64) as usize
                    } else {
                        0
                    };
                    browser.selected = pos.min(total_f.saturating_sub(1));
                    browser.clamp_selected();
                    state.modal = Some(Modal::EditDest { node_idx, browser });
                }
                return Ok(false);
            }

            // Wheel inside browser modal scrolls the picker
            if matches!(
                mouse.kind,
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            ) {
                let delta = if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                    10
                } else {
                    3
                };
                if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    browser.selected = browser.selected.saturating_sub(delta);
                } else {
                    browser.selected = (browser.selected + delta).min(total_f.saturating_sub(1));
                }
                browser.clamp_selected();
                state.modal = Some(Modal::EditDest { node_idx, browser });
                return Ok(false);
            }

            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                // Compute visible window like the draw code does
                let view_rows = modal_area.height.saturating_sub(2) as usize;
                let start = if total_f > view_rows && browser.selected >= view_rows {
                    browser.selected + 1 - view_rows
                } else {
                    0
                };

                let content_top = modal_area.y + 1;
                if mouse.row >= content_top {
                    let rel_row = (mouse.row - content_top) as usize;
                    if rel_row < view_rows && rel_row < total_f {
                        let global = start + rel_row;
                        if global < total_f {
                            browser.selected = global;
                            browser.clamp_selected();

                            // Double click detection + only on name (most of row; sb already filtered)
                            let now = Instant::now();
                            let is_double = if let Some((lx, ly, lt)) = state.last_mouse_click_pos {
                                lx == mouse.column
                                    && ly == mouse.row
                                    && now.duration_since(lt).as_millis() < 420
                            } else {
                                false
                            };
                            state.last_mouse_click_pos = Some((mouse.column, mouse.row, now));

                            // rel_x inside content (exclude right sb we already handled)
                            let rel_x = mouse.column.saturating_sub(modal_area.x + 1);
                            let content_width = modal_area.width.saturating_sub(2);
                            let is_name_part = rel_x < content_width;

                            if is_double && is_name_part {
                                // Perform the action (same paths as Enter in key handler)
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
                                        browser.clamp_selected();
                                        state.modal = Some(Modal::EditDest { node_idx, browser });
                                        return Ok(false);
                                    } else if let Some(rel_src) = selected_rel_path(state, node_idx)
                                    {
                                        let dest = browser.current.join(&entry.name);
                                        if let Err(err) = assign_destination(state, &rel_src, dest)
                                        {
                                            state.show_toast(
                                                ToastLevel::Error,
                                                format!("Failed to set destination: {err}"),
                                            );
                                        }
                                        state.modal = None;
                                        return Ok(false);
                                    }
                                }
                            }

                            // Single click: just updated selected above
                            if state.modal.is_some() {
                                state.modal = Some(Modal::EditDest { node_idx, browser });
                            }
                        }
                    }
                }
            }
        }

        Some(Modal::ImportPicker {
            files,
            selected: _selected,
        }) => {
            // Click to choose the row (Enter still required to confirm, per spec)
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let content_top = modal_area.y + 1;
                if mouse.row >= content_top {
                    let rel = (mouse.row - content_top) as usize;
                    if rel < files.len() {
                        state.modal = Some(Modal::ImportPicker {
                            files,
                            selected: rel,
                        });
                    }
                }
            }
            // Wheel could scroll but Import list is usually short; omit for minimal
        }

        Some(Modal::ConfirmBulk { .. })
        | Some(Modal::OverwriteWarning { .. })
        | Some(Modal::PreviewBulk { .. }) => {
            // Mis-click must neither apply nor dismiss a dialog that can remove_dir_all.
        }

        // For help/credits/search/ignore/export: inside click does nothing special, outside cancelled
        Some(Modal::Search)
        | Some(Modal::IgnoreEditor { .. })
        | Some(Modal::Help)
        | Some(Modal::Credits)
        | Some(Modal::ExportPicker { .. }) => {
            // no special mouse row logic
        }

        None => {}
    }

    Ok(false)
}
