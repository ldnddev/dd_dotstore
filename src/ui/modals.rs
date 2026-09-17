use crate::domain::{AppState, BulkAction, Conflict, DoctorSeverity, Modal};
use crate::ui::centered_rect;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};

pub fn overwrite_warning_counts(conflicts: &[Conflict]) -> (usize, usize, usize) {
    let dirs = conflicts.iter().filter(|c| c.is_dir).count();
    let files = conflicts.len() - dirs;
    (dirs, files, conflicts.len())
}

pub fn overwrite_conflict_row(conflict: &Conflict) -> String {
    let kind = if conflict.is_dir { "DIR" } else { "FILE" };
    format!("  {kind:<6}{}", conflict.dest.display())
}

fn preview_overwrite_counts(lines: &[String]) -> (usize, usize, usize) {
    let dirs = lines
        .iter()
        .filter(|l| l.contains("[OVERWRITE REAL DIR]"))
        .count();
    let files = lines
        .iter()
        .filter(|l| l.contains("[OVERWRITE REAL FILE]"))
        .count();
    (dirs, files, dirs + files)
}

pub(crate) fn draw_modal(f: &mut Frame, state: &mut AppState, area: Rect) {
    let modal_area = centered_rect(70, 60, area);
    state.pointer.current_modal_area = Some(modal_area);
    f.render_widget(Clear, modal_area);

    if let Some(modal) = &state.modal {
        match modal {
            Modal::Plan { action, scroll } => {
                let title_kind = match action {
                    BulkAction::Create => "Plan: Apply",
                    BulkAction::Remove => "Plan: Remove",
                };
                let preview = crate::actions::collect_preview_lines(state, *action);
                let (dirs, files, total) = preview_overwrite_counts(&preview);
                let title = if total > 0 {
                    format!("{title_kind}  ({dirs} dirs, {files} files, {total} total)")
                } else {
                    title_kind.to_string()
                };
                let inner_h = modal_area.height.saturating_sub(2) as usize;
                let list_h = inner_h.saturating_sub(2).max(1);
                let max_scroll = preview.len().saturating_sub(list_h);
                let scroll = (*scroll).min(max_scroll);
                let mut body = preview
                    .iter()
                    .skip(scroll)
                    .take(list_h)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                body.push_str("\n\nY apply / other cancel   j/k scroll");
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
            Modal::IgnoreEditor { selected, draft } => {
                let mut lines: Vec<Line<'_>> = Vec::new();
                if state.ignore_patterns.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "(no patterns — all names visible)",
                        state.theme.secondary,
                    )));
                } else {
                    for (i, pat) in state.ignore_patterns.iter().enumerate() {
                        let style = if i == *selected {
                            state.theme.selected
                        } else {
                            state.theme.modal_text
                        };
                        let marker = if i == *selected { "> " } else { "  " };
                        lines.push(Line::from(Span::styled(format!("{marker}{pat}"), style)));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("Add: {draft}█"),
                    state.theme.input_text_focus,
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "j/k move   d/Delete remove   Enter add   Esc close",
                ));
                lines.push(Line::from(
                    "Empty list disables defaults (including .dd_dotstore.json).",
                ));
                let text = Paragraph::new(lines).block(
                    Block::default()
                        .title("Ignore Editor")
                        .borders(Borders::ALL)
                        .border_style(state.theme.active_border)
                        .style(state.theme.modal),
                );
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
s / p        Plan apply (highlight or checkboxes; Y applies)\n\
x            Plan remove (highlight or checkboxes; Y applies)\n\
m            Toggle LINK/COPY for highlighted item\n\
M            Set selected items to the next LINK/COPY mode\n\
t            Set group on highlight or checkboxes\n\
T            Select every item in the highlight's group\n\
A            Reverse-import HOME/XDG symlinks that point at this project\n\
D            Doctor: broken, planned, unknown, and orphan links\n\
u            Undo last action\n\
/            Focus source filter (does not clear; also matches group names)\n\
r            Reload tree\n\
I            Ignore editor (list / add / delete, persisted)\n\
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
Click inside a confirm/overwrite/plan dialog does nothing — use Y\n\
Browser: click moves, double-click picks, scrollbar drag works\n\
\n\
Tree: proper connectors (├ └ │) + counts + subtree badges (●) for density.\n\
○ planned dest (assigned, not on disk). Unassigned rows show dim → dest.";
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
            Modal::GroupEditor { paths, draft } => {
                let text = Paragraph::new(format!(
                    "Items: {}\nGroup: {draft}█\n\nEnter save (empty clears). Esc cancel.",
                    paths.len()
                ))
                .block(
                    Block::default()
                        .title("Group")
                        .borders(Borders::ALL)
                        .border_style(state.theme.input_border_focus)
                        .style(state.theme.modal),
                )
                .style(state.theme.input_text_focus);
                f.render_widget(text, modal_area);
            }
            Modal::Adopt {
                candidates,
                selected,
                checked,
            } => {
                let mut lines: Vec<Line<'_>> =
                    vec![Line::from("Space toggle   a all   Y adopt   Esc cancel")];
                for (i, candidate) in candidates.iter().enumerate() {
                    let mark = if checked.get(i).copied().unwrap_or(false) {
                        "[✓]"
                    } else {
                        "[ ]"
                    };
                    let cursor = if i == *selected { ">" } else { " " };
                    let style = if i == *selected {
                        state.theme.selected
                    } else {
                        state.theme.modal_text
                    };
                    lines.push(Line::from(Span::styled(
                        format!(
                            "{cursor} {mark} {} -> {}",
                            candidate.src.display(),
                            candidate.dest.display()
                        ),
                        style,
                    )));
                }
                let text = Paragraph::new(lines).block(
                    Block::default()
                        .title("Adopt project symlinks")
                        .borders(Borders::ALL)
                        .border_style(state.theme.active_border)
                        .style(state.theme.modal),
                );
                f.render_widget(text, modal_area);
            }
            Modal::Doctor {
                findings,
                selected,
                scroll,
            } => {
                let view_h = modal_area.height.saturating_sub(4) as usize;
                let start = (*scroll).min(findings.len().saturating_sub(1));
                let mut lines: Vec<Line<'_>> = vec![Line::from(
                    "j/k move   Enter jump to source   A adopt orphans   Esc close",
                )];
                for (i, finding) in findings.iter().enumerate().skip(start).take(view_h.max(1)) {
                    let sev_style = match finding.severity {
                        DoctorSeverity::Critical => state.theme.error,
                        DoctorSeverity::Warning => state.theme.warning,
                        DoctorSeverity::Info => state.theme.info,
                    };
                    let row_style = if i == *selected {
                        state.theme.selected
                    } else {
                        sev_style
                    };
                    let cursor = if i == *selected { ">" } else { " " };
                    lines.push(Line::from(Span::styled(
                        format!("{cursor} {}", finding.line()),
                        row_style,
                    )));
                }
                let text = Paragraph::new(lines).block(
                    Block::default()
                        .title(format!("Doctor ({})", findings.len()))
                        .borders(Borders::ALL)
                        .border_style(state.theme.active_border)
                        .style(state.theme.modal),
                );
                f.render_widget(text, modal_area);
            }
        }
    }
}
