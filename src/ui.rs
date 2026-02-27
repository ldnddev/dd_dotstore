use crate::state::{AppState, BulkAction, Modal, Node, NodeKind, SymlinkStatus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, state, outer[0]);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(outer[1]);

    draw_source_panel(f, state, chunks[0]);
    draw_status_panel(f, state, chunks[1]);
    draw_status_bar(f, outer[2]);

    if state.modal.is_some() {
        draw_modal(f, state, f.area());
    }
}

fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let header = Paragraph::new(state.header_copy.as_str()).block(
        Block::default()
            .title("dd_dotstore")
            .borders(Borders::ALL)
            .border_style(state.theme.border),
    );
    f.render_widget(header, area);
}

fn draw_status_bar(f: &mut Frame, area: Rect) {
    let bar =
        Paragraph::new("F1: Help   F2: Credits   /: Search   i: Import   E: Export   Q: Exit")
            .block(Block::default())
            .style(ratatui::style::Style::default());
    f.render_widget(bar, area);
}

fn draw_source_panel(f: &mut Frame, state: &mut AppState, area: Rect) {
    let items: Vec<ListItem<'_>> = state
        .nodes
        .iter()
        .map(|node| {
            let (icon, icon_style) = match node.symlink_status {
                SymlinkStatus::Valid => ("✓ ", state.theme.valid),
                SymlinkStatus::Broken => ("✗ ", state.theme.broken),
                SymlinkStatus::Unknown => ("? ", state.theme.highlight),
                SymlinkStatus::None => ("  ", state.theme.normal),
            };

            let prefix = if node.selected { "[✓] " } else { "[ ] " };

            let name = match &node.kind {
                NodeKind::Folder { expanded, .. } => {
                    if *expanded {
                        format!("> {}", node.name)
                    } else {
                        format!("+ {}", node.name)
                    }
                }
                _ => node.name.clone(),
            };

            let content = Line::from(vec![
                Span::styled(prefix, state.theme.normal),
                Span::styled(icon, icon_style),
                Span::raw(name),
            ]);

            ListItem::new(content)
        })
        .collect();

    let title = if state.filter.is_empty() {
        "Source".to_string()
    } else {
        format!("Source (filter: {})", state.filter)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(state.theme.border),
        )
        .highlight_style(state.theme.selected);

    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn draw_status_panel(f: &mut Frame, state: &mut AppState, area: Rect) {
    let mut lines = vec![];

    fn collect_symlinks(node: &Node, out: &mut Vec<String>) {
        if let NodeKind::File { dest: Some(d) } = &node.kind {
            out.push(format!("• {} -> {}", node.path.display(), d.display()));
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            for c in children {
                collect_symlinks(c, out);
            }
        }
    }

    for node in &state.tree {
        collect_symlinks(node, &mut lines);
    }

    let items: Vec<ListItem<'_>> = lines.into_iter().map(ListItem::new).collect();

    let list = List::new(items).block(
        Block::default()
            .title("Symlinks")
            .borders(Borders::ALL)
            .border_style(state.theme.border),
    );

    f.render_stateful_widget(list, area, &mut state.status_list_state);
}

fn draw_modal(f: &mut Frame, state: &AppState, area: Rect) {
    let modal_area = centered_rect(70, 60, area);
    f.render_widget(Clear, modal_area);

    if let Some(modal) = &state.modal {
        match modal {
            Modal::ConfirmBulk { action } => {
                let title = match action {
                    BulkAction::Create => "Create symlinks?",
                    BulkAction::Remove => "Remove symlinks?",
                };
                let text = Paragraph::new(
                    "Confirm action for selected items?\nY = yes, any other key = cancel",
                )
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(state.theme.border),
                );
                f.render_widget(text, modal_area);
            }
            Modal::OverwriteWarning { conflicts, .. } => {
                let real_count = conflicts.iter().filter(|c| c.is_real_file).count();
                let symlink_count = conflicts.len() - real_count;
                let mut names = String::new();
                for conflict in conflicts.iter().take(4) {
                    if !names.is_empty() {
                        names.push('\n');
                    }
                    names.push_str(&conflict.dest.display().to_string());
                }
                let msg = format!(
                    "{} conflicts: {} real files, {} symlinks.\n{}",
                    conflicts.len(),
                    real_count,
                    symlink_count,
                    names
                );
                let text = Paragraph::new(msg).block(
                    Block::default()
                        .title("Overwrite Warning")
                        .borders(Borders::ALL)
                        .border_style(state.theme.error),
                );
                f.render_widget(text, modal_area);
            }
            Modal::Error { msg } => {
                let text = Paragraph::new(msg.as_str()).block(
                    Block::default()
                        .title("Notice")
                        .borders(Borders::ALL)
                        .border_style(state.theme.error),
                );
                f.render_widget(text, modal_area);
            }
            Modal::Search => {
                let text = Paragraph::new(format!("Filter: {}", state.filter))
                    .block(Block::default().title("Search").borders(Borders::ALL));
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
                        Some(ListItem::new(format!("{}{}", e.name, suffix)))
                    })
                    .collect();
                let title = format!(
                    "Edit Destination: {} | filter: {} (type to fuzzy filter, g/G/~, Ctrl+S: use dir)",
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
                            .border_style(state.theme.border)
                            .style(state.theme.normal),
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
                        .end_symbol(None);
                    f.render_stateful_widget(scrollbar, modal_area, &mut scrollbar_state);
                }
            }
            Modal::IgnoreEditor { .. } => {
                let text = Paragraph::new("Use j/k and d to edit ignores. Esc to close.").block(
                    Block::default()
                        .title("Ignore Editor")
                        .borders(Borders::ALL),
                );
                f.render_widget(text, modal_area);
            }
            Modal::Help => {
                let help = "Keybindings\n\
\n\
q / Esc      Quit\n\
F1           Toggle help\n\
F2           Toggle credits\n\
j/k or ↑/↓   Navigate\n\
Space        Expand/collapse folder or toggle file select\n\
Enter / e    Edit destination (file)\n\
s            Bulk create selected symlinks\n\
x            Bulk remove selected symlinks\n\
u            Undo last action\n\
/            Search/filter\n\
r            Reload tree\n\
I            Ignore editor\n\
i            Import picker\n\
E            Export picker\n";
                let text = Paragraph::new(help).block(
                    Block::default()
                        .title("Help")
                        .borders(Borders::ALL)
                        .border_style(state.theme.border),
                );
                f.render_widget(text, modal_area);
            }
            Modal::Credits => {
                let credits = "Credits\n\
\n\
- Catppuccin: color inspiration\n\
- GNU Stow: dotfile workflow inspiration\n\
- Ratatui + Crossterm: TUI stack\n\
\n\
Press Esc/F2 to close.";
                let text = Paragraph::new(credits).block(
                    Block::default()
                        .title("Credits")
                        .borders(Borders::ALL)
                        .border_style(state.theme.border),
                );
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
                let list = List::new(items).block(
                    Block::default()
                        .title("Import Picker (newest first; j/k, Enter, Esc)")
                        .borders(Borders::ALL)
                        .border_style(state.theme.border),
                );
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
                        .border_style(state.theme.border),
                );
                f.render_widget(text, modal_area);
            }
        }
    }
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
