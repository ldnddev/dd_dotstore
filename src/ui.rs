// ui.rs

use crate::app::{App, ActiveTab};
use ratatui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
        .split(frame.area());

    let tabs = ratatui::widgets::Tabs::new(vec!["List", "Symlink"])
        .select(if app.active_tab == ActiveTab::List { 0 } else { 1 });
    frame.render_widget(tabs, chunks[0]);

    match app.active_tab {
        ActiveTab::List => render_list_tab(frame, app, chunks[1]),
        ActiveTab::Symlink => render_symlink_tab(frame, app, chunks[1]),
    }
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
