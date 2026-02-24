// ui.rs

use crate::app::{App, ActiveTab};
use ratatui::{
    backend::Backend,
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, app: &App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(90), Constraint::Length(1)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(main_chunks[0]);

    let tabs = ratatui::widgets::Tabs::new(vec!["List", "Symlink"])
        .select(if app.active_tab == ActiveTab::List { 0 } else { 1 })
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(tabs, chunks[0]);

    match app.active_tab {
        ActiveTab::List => render_list_tab(frame, app, chunks[1]),
        ActiveTab::Symlink => render_symlink_tab(frame, app, chunks[1]),
    }

    render_status_bar(frame, app, main_chunks[1]);

    if app.show_keybindings {
        render_keybindings_modal(frame, app, main_chunks[0]);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let text = match app.active_tab {
        ActiveTab::List => "A: Add | E: Edit | R: Remove | Tab: Switch | F1: Help | Q: Quit",
        ActiveTab::Symlink => "S: Create Symlinks | Tab: Switch | F1: Help | Q: Quit",
    };
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_keybindings_modal(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(area, 60, 40);
    frame.render_widget(Clear, popup);

    let lines = vec![
        Line::from("Keybindings"),
        Line::from(""),
        Line::from("Global:"),
        Line::from("Tab: Switch tabs"),
        Line::from("F1: Toggle this help modal"),
        Line::from("Q: Quit"),
        Line::from(""),
        Line::from("List Tab:"),
        Line::from("Up/Down: Select item"),
        Line::from("A: Add new dotfile (prompt name then target)"),
        Line::from("E: Edit selected (prompt name then target)"),
        Line::from("R: Remove selected"),
        Line::from(""),
        Line::from("Symlink Tab:"),
        Line::from("S: Create symlinks (prompt repo path, confirm)"),
        Line::from(""),
        Line::from("Input Mode (when prompting):"),
        Line::from("Enter: Confirm/Next field"),
        Line::from("Esc: Cancel"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title("Help (F1 to close)").borders(Borders::ALL))
        .wrap(Wrap::default());
    frame.render_widget(paragraph, popup);
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
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


fn render_list_tab<B: Backend>(frame: &mut Frame<B>, app: &App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = app.dotfiles.iter().enumerate().map(|(i, d)| {
        let line = format!("{} -> {}", d.name, d.target);
        ListItem::new(line).style(if Some(i) == app.selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        })
    }).collect();
    let list = List::new(items).block(Block::default().title("Dotfiles").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn render_symlink_tab<B: Backend>(frame: &mut Frame<B>, app: &App, area: ratatui::layout::Rect) {
    let text = "Press S to create symlinks";
    let paragraph = Paragraph::new(text).block(Block::default().title("Symlink").borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}
