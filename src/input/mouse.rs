use anyhow::Result;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::time::Instant;

use crate::actions::{
    assign_destination, collect_preview_lines, revert_theme_editor, theme_editor_select,
};
use crate::domain::{AppState, Modal, NodeKind};
use crate::input::hit_test::{
    hit_test_source_row, is_over_scrollbar, rect_contains, zones_for_node,
};
use crate::input::keys::{open_dest_browser_for_idx, selected_rel_path};
use crate::tree::{find_mut_node, flatten_visible, toggle_expand, toggle_selected};
use crate::ui::toast::ToastLevel;

pub fn handle_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    // Clear transient drag on button up regardless
    if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
        state.pointer.scrollbar_dragging = false;
    }

    // Toast click-to-dismiss (any button)
    if let Some(tarea) = state.pointer.toast_area
        && rect_contains(tarea, mouse.column, mouse.row)
    {
        state.toast = None;
        state.pointer.toast_area = None;
        return Ok(false);
    }

    if state.modal.is_some() {
        return handle_modal_mouse(state, mouse);
    }

    handle_main_mouse(state, mouse)
}

fn handle_main_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let delta: usize = if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                8
            } else {
                3
            };
            if rect_contains(state.pointer.source_area, mouse.column, mouse.row) {
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
                let vh = state.pointer.source_area.height.saturating_sub(2) as usize;
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
            } else if rect_contains(state.pointer.status_area, mouse.column, mouse.row) {
                // Wheel over Destinations panel: scroll the right panel for information density
                let total = state.pointer.destination_paths.len();
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
            if !rect_contains(state.pointer.source_area, mouse.column, mouse.row)
                && !rect_contains(state.pointer.status_area, mouse.column, mouse.row)
            {
                return Ok(false);
            }

            // Scrollbar drag/click takes precedence (for both panels)
            let over_source_sb =
                is_over_scrollbar(&state.pointer.source_area, mouse.column, mouse.row);
            let over_status_sb =
                is_over_scrollbar(&state.pointer.status_area, mouse.column, mouse.row);
            if over_source_sb || over_status_sb || state.pointer.scrollbar_dragging {
                state.pointer.scrollbar_dragging = true;
                // During active drag, decide panel by which rect the mouse row is in (allows dragging off the thin sb column)
                if rect_contains(state.pointer.status_area, mouse.column, mouse.row) {
                    update_status_scrollbar(state, mouse.row);
                } else if rect_contains(state.pointer.source_area, mouse.column, mouse.row) {
                    update_source_scrollbar(state, mouse.row);
                } else if state.pointer.status_area.height > 0 {
                    // Fallback: if mouse y roughly in status vertical range
                    let in_status_y = mouse.row >= state.pointer.status_area.y
                        && mouse.row
                            < state.pointer.status_area.y + state.pointer.status_area.height;
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

            if rect_contains(state.pointer.source_area, mouse.column, mouse.row)
                && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            {
                // Double-click detection (only counts on name part later)
                let now = Instant::now();
                let is_double = if let Some((lx, ly, lt)) = state.pointer.last_mouse_click_pos {
                    lx == mouse.column
                        && ly == mouse.row
                        && now.duration_since(lt).as_millis() < 420
                } else {
                    false
                };
                state.pointer.last_mouse_click_pos = Some((mouse.column, mouse.row, now));

                if let Some((idx, rel_x)) = hit_test_source_row(state, mouse.column, mouse.row) {
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
                    } else if let Some(node) = state.nodes.get(idx).cloned() {
                        let zones = zones_for_node(state, &node);
                        let is_folder = matches!(node.kind, NodeKind::Folder { .. });
                        if zones.checkbox.contains(&rel_x) {
                            toggle_selection_at(state, idx);
                        } else if is_folder && zones.tree.contains(&rel_x) {
                            toggle_expand_at(state, idx);
                        } else if is_double && zones.name.contains(&rel_x) {
                            open_dest_browser_for_idx(state, idx);
                        }
                    }
                }
                // (no else for drag comment here, as drag is handled before this if)
            } else if rect_contains(state.pointer.status_area, mouse.column, mouse.row) {
                // Click in Destinations panel: jump highlight to the corresponding source item (polish + navigation)
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                    let content_top = state.pointer.status_area.y + 1;
                    if mouse.row >= content_top {
                        let rel = (mouse.row - content_top) as usize;
                        let offset = state.status_list_state.offset();
                        let global = offset + rel;
                        if global < state.pointer.destination_paths.len() {
                            let target_path = state.pointer.destination_paths[global].clone();
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
            state.pointer.scrollbar_dragging = false;
        }

        _ => {}
    }
    Ok(false)
}

fn update_source_scrollbar(state: &mut AppState, mouse_row: u16) {
    let area = state.pointer.source_area;
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
    let area = state.pointer.status_area;
    let total = state.pointer.destination_paths.len();
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

fn handle_modal_mouse(state: &mut AppState, mouse: MouseEvent) -> Result<bool> {
    let Some(modal_area) = state.pointer.current_modal_area else {
        // No area yet (shouldn't happen), fall back to cancel on any click
        state.modal = None;
        return Ok(false);
    };

    let inside_modal = rect_contains(modal_area, mouse.column, mouse.row);
    if !inside_modal {
        // Outside click cancels the modal (Esc semantics for most)
        if matches!(state.modal, Some(Modal::ThemeEditor(_))) {
            revert_theme_editor(state);
        }
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
                            let is_double =
                                if let Some((lx, ly, lt)) = state.pointer.last_mouse_click_pos {
                                    lx == mouse.column
                                        && ly == mouse.row
                                        && now.duration_since(lt).as_millis() < 420
                                } else {
                                    false
                                };
                            state.pointer.last_mouse_click_pos =
                                Some((mouse.column, mouse.row, now));

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

        Some(Modal::Adopt {
            candidates,
            selected: _selected,
            checked,
        }) => {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let content_top = modal_area.y + 2;
                if mouse.row >= content_top {
                    let rel = (mouse.row - content_top) as usize;
                    if rel < candidates.len() {
                        state.modal = Some(Modal::Adopt {
                            candidates,
                            selected: rel,
                            checked,
                        });
                    }
                }
            }
        }

        Some(Modal::Doctor {
            findings,
            selected: _selected,
            scroll,
        }) => {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let content_top = modal_area.y + 2;
                if mouse.row >= content_top {
                    let rel = (mouse.row - content_top) as usize + scroll;
                    if rel < findings.len() {
                        state.modal = Some(Modal::Doctor {
                            findings,
                            selected: rel,
                            scroll,
                        });
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

        Some(Modal::Plan { action, scroll }) => {
            // Inside click must neither apply nor dismiss; wheel may scroll.
            if matches!(
                mouse.kind,
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            ) {
                let max_scroll = collect_preview_lines(state, action).len().saturating_sub(1);
                let scroll = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                    scroll.saturating_sub(1)
                } else {
                    (scroll + 1).min(max_scroll)
                };
                state.modal = Some(Modal::Plan { action, scroll });
            }
        }
        Some(Modal::OverwriteWarning { .. }) => {
            // Mis-click must neither apply nor dismiss a dialog that can remove_dir_all.
        }

        // For help/credits/ignore/export: inside click does nothing special, outside cancelled
        Some(Modal::ThemeEditor(editor)) => {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let content_top = modal_area.y + 4;
                if mouse.row >= content_top {
                    let rel = (mouse.row - content_top) as usize;
                    let rows = crate::theme::theme_editor_rows();
                    let start = rows
                        .iter()
                        .position(|row| match row {
                            crate::theme::ThemeEditorRow::Color(idx) => *idx >= editor.scroll,
                            crate::theme::ThemeEditorRow::Header(_) => false,
                        })
                        .unwrap_or(0);
                    let start = if start > 0
                        && matches!(rows[start - 1], crate::theme::ThemeEditorRow::Header(_))
                    {
                        start - 1
                    } else {
                        start
                    };
                    if let Some(crate::theme::ThemeEditorRow::Color(idx)) = rows.get(start + rel) {
                        theme_editor_select(state, *idx);
                    }
                }
            }
        }

        Some(Modal::IgnoreEditor { .. })
        | Some(Modal::Help)
        | Some(Modal::Credits)
        | Some(Modal::ExportPicker { .. })
        | Some(Modal::GroupEditor { .. }) => {
            // no special mouse row logic
        }

        None => {}
    }

    Ok(false)
}
