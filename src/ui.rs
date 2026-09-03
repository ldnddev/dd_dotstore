use crate::state::{
    ActionMode, AppState, BulkAction, Conflict, Modal, Node, NodeKind, SymlinkStatus,
};
use crate::toast::ToastLevel;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};
use std::path::PathBuf;

pub fn draw(f: &mut Frame, state: &mut AppState) {
    state.last_frame_area = f.area();
    if state.toast.is_none() {
        state.toast_area = None;
    }
    if state.modal.is_none() {
        state.current_modal_area = None;
    }
    f.render_widget(Block::default().style(state.theme.app_shell), f.area());

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1), // footer is now just the key hints line (decluttered)
        ])
        .split(f.area());

    draw_header(f, state, outer[0]);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[1]);

    draw_source_panel(f, state, chunks[0]);
    draw_status_panel(f, state, chunks[1]);
    draw_status_bar(f, state, outer[2]);

    if state.modal.is_some() {
        draw_modal(f, state, f.area());
    }

    if state.toast.is_some() {
        draw_toast(f, state, f.area());
    }
}

fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let header = Paragraph::new(state.header_copy.as_str()).block(
        Block::default()
            .title("dd_dotstore")
            .borders(Borders::ALL)
            .border_style(state.theme.active_border)
            .style(state.theme.app_shell),
    );
    f.render_widget(header, area);
}

fn draw_status_bar(f: &mut Frame, state: &AppState, area: Rect) {
    // Width-adaptive key hints (footer reduced to 1 line; theme status moved out of persistent footer
    // to reduce clutter — full details still in F2 Credits and at startup).
    let keys = if area.width < 75 {
        "F1:Help  q:Quit  j/k:Nav  Spc:Sel  s:Apply  x:Rem  /:Filter"
    } else if area.width < 110 {
        "F1: Help   /: Search   Space: Select   m/M: Link/Copy   s: Apply   x: Remove   q: Quit"
    } else {
        "F1: Help   /: Search   Space: Select   m/M: Link/Copy   s: Apply   x: Remove   q: Quit   (mouse: click/scroll/drag)"
    };

    let bar = Paragraph::new(Line::from(keys))
        .block(Block::default())
        .style(state.theme.app_shell);
    f.render_widget(bar, area);
}

fn subtree_has_destination(node: &Node) -> bool {
    match &node.kind {
        NodeKind::File { dest: Some(_) } | NodeKind::Folder { dest: Some(_), .. } => true,
        NodeKind::Folder { children, .. } => children.iter().any(subtree_has_destination),
        _ => false,
    }
}

fn draw_source_panel(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.source_area = area;
    let items: Vec<ListItem<'_>> = state
        .nodes
        .iter()
        .map(|node| {
            let mut icon = match node.symlink_status {
                SymlinkStatus::Valid => "✓ ",
                SymlinkStatus::Broken => "✗ ",
                SymlinkStatus::Unknown => "? ",
                SymlinkStatus::None => "  ",
            };
            let mut icon_style = match node.symlink_status {
                SymlinkStatus::Valid => state.theme.valid,
                SymlinkStatus::Broken => state.theme.broken,
                SymlinkStatus::Unknown => state.theme.highlight,
                SymlinkStatus::None => state.theme.normal,
            };
            // Enhanced visibility: folders containing configured descendants get a subtle badge icon
            if matches!(&node.kind, NodeKind::Folder { dest: None, .. })
                && subtree_has_destination(node)
                && node.symlink_status == SymlinkStatus::None
            {
                icon = "◌ ";
                icon_style = state.theme.secondary;
            }

            let prefix = if node.selected { "[✓] " } else { "[ ] " };
            let mode_style = match node.action_mode {
                ActionMode::Symlink => state.theme.valid,
                ActionMode::Copy => state.theme.highlight,
            };

            let (mut name, name_style) = match &node.kind {
                NodeKind::Folder { .. } => (node.name.clone(), state.theme.folder),
                _ => (node.name.clone(), state.theme.file),
            };
            // Badge for folders that have (grand)children with destinations configured (information density)
            if matches!(&node.kind, NodeKind::Folder { dest: None, .. })
                && subtree_has_destination(node)
            {
                name.push_str(" ●");
            }

            let content = Line::from(vec![
                Span::styled(prefix, state.theme.normal),
                Span::styled(icon, icon_style),
                Span::styled(format!("[{}] ", node.action_mode.label()), mode_style),
                Span::styled(name, name_style),
            ]);

            ListItem::new(content)
        })
        .collect();

    let total = state.nodes.len();
    let selected_count = state.nodes.iter().filter(|n| n.selected).count();
    let base_title = if state.filter.is_empty() {
        "Source".to_string()
    } else {
        format!("Source (filter: {})", state.filter)
    };
    let title = if selected_count > 0 {
        format!("{}  [{} selected / {}]", base_title, selected_count, total)
    } else {
        format!("{}  [{}]", base_title, total)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(state.theme.active_border)
                .style(state.theme.body),
        )
        .highlight_style(state.theme.selected);

    f.render_stateful_widget(list, area, &mut state.list_state);

    // Render scrollbar on the right edge when content overflows (supports mouse drag to scroll)
    let total = state.nodes.len();
    let view_rows = area.height.saturating_sub(2) as usize;
    if total > view_rows && view_rows > 0 {
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(state.list_state.offset())
            .viewport_content_length(view_rows);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .thumb_style(state.theme.scrollbar);
        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn draw_status_panel(f: &mut Frame, state: &mut AppState, area: Rect) {
    state.status_area = area;
    let mut lines: Vec<Line<'_>> = vec![];
    state.destination_paths.clear();

    fn collect_destinations<'a>(
        node: &Node,
        out: &mut Vec<Line<'a>>,
        paths: &mut Vec<PathBuf>,
        theme: &crate::state::Theme,
    ) {
        match &node.kind {
            NodeKind::File { dest: Some(d) } | NodeKind::Folder { dest: Some(d), .. } => {
                let arrow = match node.action_mode {
                    ActionMode::Symlink => "->",
                    ActionMode::Copy => "=>",
                };
                let mode_style = match node.action_mode {
                    ActionMode::Symlink => theme.link,
                    ActionMode::Copy => theme.warning,
                };
                let kind_style = match node.kind {
                    NodeKind::Folder { .. } => theme.folder,
                    NodeKind::File { .. } => theme.file,
                };
                out.push(Line::from(vec![
                    Span::styled("• ", mode_style),
                    Span::styled(node.action_mode.label().to_string(), mode_style),
                    Span::raw(" "),
                    Span::styled(node.path.display().to_string(), kind_style),
                    Span::raw(format!(" {arrow} ")),
                    Span::styled(d.display().to_string(), mode_style),
                ]));
                paths.push(node.path.clone());
            }
            NodeKind::File { dest: None } | NodeKind::Folder { dest: None, .. } => {}
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            for c in children {
                collect_destinations(c, out, paths, theme);
            }
        }
    }

    for node in &state.tree {
        collect_destinations(node, &mut lines, &mut state.destination_paths, &state.theme);
    }

    let dest_count = lines.len();
    let title = format!("Destinations ({})", dest_count);

    let items: Vec<ListItem<'_>> = lines.into_iter().map(ListItem::new).collect();

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(state.theme.border)
            .style(state.theme.body),
    );

    f.render_stateful_widget(list, area, &mut state.status_list_state);

    // Add scrollbar for the destinations panel (information density + mouse drag support)
    let total = dest_count;
    let view_rows = area.height.saturating_sub(2) as usize;
    if total > view_rows && view_rows > 0 {
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(state.status_list_state.offset())
            .viewport_content_length(view_rows);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .thumb_style(state.theme.scrollbar);
        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn draw_modal(f: &mut Frame, state: &mut AppState, area: Rect) {
    let modal_area = centered_rect(70, 60, area);
    state.current_modal_area = Some(modal_area);
    f.render_widget(Clear, modal_area);

    if let Some(modal) = &state.modal {
        match modal {
            Modal::ConfirmBulk { action } => {
                let title = match action {
                    BulkAction::Create => "Confirm: Apply selected?",
                    BulkAction::Remove => "Confirm: Remove selected?",
                };
                let preview = crate::actions::collect_preview_lines(state, *action);
                let mut body = preview.join("\n");
                body.push_str("\n\nY/Enter = proceed, other = cancel (safety preview)");
                let text = Paragraph::new(body)
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_style(state.theme.active_border)
                            .style(state.theme.modal),
                    )
                    .style(state.theme.modal_text);
                f.render_widget(text, modal_area);
            }
            Modal::PreviewBulk { action } => {
                let title = match action {
                    BulkAction::Create => "PREVIEW: Planned CREATE (dry-run)",
                    BulkAction::Remove => "PREVIEW: Planned REMOVE (dry-run)",
                };
                let preview = crate::actions::collect_preview_lines(state, *action);
                let mut body = preview.join("\n");
                body.push_str("\n\nThis is a DRY RUN / safety preview.\nY/Enter = actually apply now, other key = close (no changes)");
                let text = Paragraph::new(body)
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_style(state.theme.active_border)
                            .style(state.theme.modal),
                    )
                    .style(state.theme.modal_text);
                f.render_widget(text, modal_area);
            }
            Modal::OverwriteWarning {
                conflicts, scroll, ..
            } => {
                let (dirs, files, total) = overwrite_warning_counts(conflicts);
                let title =
                    format!("Overwrite Warning  ({dirs} dirs, {files} files, {total} total)");
                let inner_h = modal_area.height.saturating_sub(2) as usize;
                let list_h = inner_h.saturating_sub(4).max(1);
                let max_scroll = conflicts.len().saturating_sub(list_h);
                let scroll = (*scroll).min(max_scroll);
                let mut body = String::from(
                    "These real paths will be replaced. Directories will be DELETED (remove_dir_all):\n\n",
                );
                for conflict in conflicts.iter().skip(scroll).take(list_h) {
                    body.push_str(&overwrite_conflict_row(conflict));
                    body.push('\n');
                }
                body.push_str("\nY apply / other cancel   j/k scroll");
                let text = Paragraph::new(body)
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_style(state.theme.error)
                            .style(state.theme.modal),
                    )
                    .style(state.theme.modal_text);
                f.render_widget(text, modal_area);
            }
            Modal::Search => {
                let text = Paragraph::new(format!("Filter: {}", state.filter))
                    .style(state.theme.input_text_focus)
                    .block(
                        Block::default()
                            .title("Search")
                            .borders(Borders::ALL)
                            .border_style(state.theme.input_border_focus)
                            .style(state.theme.modal),
                    );
                f.render_widget(text, modal_area);
            }
            Modal::EditDest { browser, .. } => {
                let filtered = browser.filtered_indices();
                let total = filtered.len();
                let view_rows = modal_area.height.saturating_sub(2) as usize;
                let start = if total > view_rows && browser.selected >= view_rows {
                    browser.selected + 1 - view_rows
                } else {
                    0
                };

                let items: Vec<ListItem<'_>> = (0..view_rows)
                    .filter_map(|row| {
                        let global = start + row;
                        let entry_idx = *filtered.get(global)?;
                        let e = browser.entries.get(entry_idx)?;
                        let suffix = if e.is_dir { "/" } else { "" };
                        let style = if e.is_dir {
                            state.theme.folder
                        } else {
                            state.theme.file
                        };
                        Some(ListItem::new(Line::from(Span::styled(
                            format!("{}{}", e.name, suffix),
                            style,
                        ))))
                    })
                    .collect();
                let title = format!(
                    "Edit Destination: {} | filter: {} (type fuzzy, g/G/~, Ctrl+S, click/dbl-click, scrollbar drag)",
                    browser.current.display(),
                    if browser.filter.is_empty() {
                        "<none>"
                    } else {
                        &browser.filter
                    }
                );
                let list = List::new(items)
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_style(state.theme.input_border_focus)
                            .style(state.theme.modal),
                    )
                    .highlight_style(state.theme.selected)
                    .highlight_symbol("> ");

                let mut list_state = ListState::default();
                if browser.selected >= start && browser.selected < start.saturating_add(view_rows) {
                    list_state.select(Some(browser.selected - start));
                }
                f.render_stateful_widget(list, modal_area, &mut list_state);

                if total > view_rows && view_rows > 0 {
                    let mut scrollbar_state = ScrollbarState::new(total)
                        .position(start)
                        .viewport_content_length(view_rows);
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(None)
                        .end_symbol(None)
                        .thumb_style(state.theme.scrollbar);
                    f.render_stateful_widget(scrollbar, modal_area, &mut scrollbar_state);
                }
            }
            Modal::IgnoreEditor { .. } => {
                let text = Paragraph::new("Use j/k and d to edit ignores. Esc to close.")
                    .block(
                        Block::default()
                            .title("Ignore Editor")
                            .borders(Borders::ALL)
                            .border_style(state.theme.active_border)
                            .style(state.theme.modal),
                    )
                    .style(state.theme.modal_text);
                f.render_widget(text, modal_area);
            }
            Modal::Help => {
                let help = "Keybindings\n\
\n\
q / Q        Quit\n\
Esc          Close modal / clear filter / clear selection (does not quit)\n\
F1           Toggle help\n\
F2           Toggle credits\n\
j/k or ↑/↓   Navigate\n\
Space        Toggle selection (file/folder)\n\
Enter        Edit destination (file/folder)\n\
e            Edit destination (file/folder)\n\
h/l or ←/→   Collapse/expand folder\n\
s            Apply selected (confirm)\n\
x            Remove selected (confirm)\n\
p            Preview/dry-run plan for apply (Y to proceed)\n\
m            Toggle LINK/COPY for highlighted item\n\
M            Set selected items to the next LINK/COPY mode\n\
u            Undo last action\n\
/            Search/filter\n\
r            Reload tree\n\
I            Ignore editor\n\
i            Import picker\n\
E            Export picker\n\
\n\
Mouse\n\
Click row           Highlight\n\
Far-left click      Toggle [ ] select\n\
Click tree area     Toggle expand (folders)\n\
Shift+click         Range multi-select from current to clicked\n\
Double-click name   Edit destination\n\
Wheel over list     Scroll source (Shift=faster)\n\
Wheel over right    Scroll Destinations\n\
Drag right scrollbar Scroll the view\n\
Click right panel   Jump focus + auto-expand ancestors\n\
Click outside modal Close it\n\
Click inside confirm/overwrite does nothing — use Y\n\
Browser: click moves, double-click picks, scrollbar drag works\n\
\n\
Tree: proper connectors (├ └ │) + counts + subtree badges (●) for density.";
                let text = Paragraph::new(help)
                    .block(
                        Block::default()
                            .title("Help")
                            .borders(Borders::ALL)
                            .border_style(state.theme.active_border)
                            .style(state.theme.modal),
                    )
                    .style(state.theme.modal_text);
                f.render_widget(text, modal_area);
            }
            Modal::Credits => {
                let credits = format!(
                    "Credits\n\
\n\
- GNU Stow: dotfile workflow inspiration\n\
- Ratatui + Crossterm: TUI stack\n\
\n\
Theme source: {}\n\
Theme status: {}\n\
\n\
Press Esc/F2 to close.",
                    state.theme.source.label(),
                    state.theme_status.message
                );
                let text = Paragraph::new(credits)
                    .block(
                        Block::default()
                            .title("Credits")
                            .borders(Borders::ALL)
                            .border_style(state.theme.active_border)
                            .style(state.theme.modal),
                    )
                    .style(state.theme.modal_text);
                f.render_widget(text, modal_area);
            }
            Modal::ImportPicker { files, selected } => {
                let items: Vec<ListItem<'_>> = files
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let cursor = if i == *selected { "> " } else { "  " };
                        let name = p
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| p.display().to_string());
                        let modified = std::fs::metadata(p)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        ListItem::new(format!(
                            "{cursor}{name}  [mtime:{modified}]  {}",
                            p.display()
                        ))
                    })
                    .collect();
                let list = List::new(items)
                    .block(
                        Block::default()
                            .title("Import Picker (newest first; j/k/click, Enter, Esc)")
                            .borders(Borders::ALL)
                            .border_style(state.theme.active_border)
                            .style(state.theme.modal),
                    )
                    .highlight_style(state.theme.selected);
                f.render_widget(list, modal_area);
            }
            Modal::ExportPicker { dir, filename } => {
                let text = Paragraph::new(format!(
                    "Directory: {}\nFilename: {}\n\nType to edit, Enter to export, Esc to cancel.",
                    dir.display(),
                    filename
                ))
                .block(
                    Block::default()
                        .title("Export Picker")
                        .borders(Borders::ALL)
                        .border_style(state.theme.input_border_focus)
                        .style(state.theme.modal),
                )
                .style(state.theme.input_text_focus);
                f.render_widget(text, modal_area);
            }
        }
    }
}

fn draw_toast(f: &mut Frame, state: &mut AppState, area: Rect) {
    let Some(toast) = &state.toast else {
        state.toast_area = None;
        return;
    };
    if area.width < 8 || area.height < 5 {
        state.toast_area = None;
        return;
    }

    let max_width = area.width.saturating_sub(2).min(50);
    let longest_line = toast
        .message
        .lines()
        .map(|line| line.chars().count() as u16)
        .max()
        .unwrap_or(0);
    let width = longest_line.saturating_add(4).clamp(24, max_width);
    let line_count = toast.message.lines().count().max(1) as u16;
    let height = line_count
        .saturating_add(2)
        .clamp(3, area.height.saturating_sub(1).min(7));
    let x = area.x + area.width.saturating_sub(width).saturating_sub(1);
    let y = area.y + area.height.saturating_sub(height).saturating_sub(1);
    let toast_area = Rect::new(x, y, width, height);
    state.toast_area = Some(toast_area);

    let (title, border_style) = match toast.level {
        ToastLevel::Info => ("Info", state.theme.info),
        ToastLevel::Success => ("Success", state.theme.valid),
        ToastLevel::Warning => ("Warning", state.theme.warning),
        ToastLevel::Error => ("Error", state.theme.error),
    };

    let text = Paragraph::new(toast.message.as_str())
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(state.theme.modal),
        )
        .style(state.theme.modal_text)
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, toast_area);
    f.render_widget(text, toast_area);
}

pub fn overwrite_warning_counts(conflicts: &[Conflict]) -> (usize, usize, usize) {
    let dirs = conflicts.iter().filter(|c| c.is_dir).count();
    let files = conflicts.len() - dirs;
    (dirs, files, conflicts.len())
}

pub fn overwrite_conflict_row(conflict: &Conflict) -> String {
    let kind = if conflict.is_dir { "DIR" } else { "FILE" };
    format!("  {kind:<6}{}", conflict.dest.display())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
