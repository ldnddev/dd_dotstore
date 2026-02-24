// main.rs

use std::io;
use crossterm::{
    event::{self, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use app::{App, ActiveTab, InputMode};
use ui::render;

mod app;
mod ui;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        terminal.draw(|f| render(f, &app))?;

        if let event::Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::None => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => app.active_tab = if app.active_tab == ActiveTab::List { ActiveTab::Symlink } else { ActiveTab::List },
                    KeyCode::Up if app.active_tab == ActiveTab::List => app.selected = app.selected.map(|i| i.saturating_sub(1)),
                    KeyCode::Down if app.active_tab == ActiveTab::List => {
                        app.selected = if let Some(i) = app.selected {
                            if i + 1 < app.dotfiles.len() { Some(i + 1) } else { Some(0) }
                        } else if !app.dotfiles.is_empty() { Some(0) } else { None };
                    },
                    KeyCode::Char('a') if app.active_tab == ActiveTab::List => { app.input_mode = InputMode::Name; app.current_name.clear(); app.current_target.clear(); },
                    KeyCode::Char('r') if app.active_tab == ActiveTab::List => if let Some(i) = app.selected { app.remove_dotfile(i) },
                    KeyCode::Char('e') if app.active_tab == ActiveTab::List => if let Some(i) = app.selected { 
                        app.current_name = app.dotfiles[i].name.clone(); 
                        app.current_target = app.dotfiles[i].target.clone(); 
                        app.input_mode = InputMode::Name; 
                    },
                    KeyCode::Char('s') if app.active_tab == ActiveTab::Symlink => { app.create_symlinks("/path/to/repo")?; },
                    _ => {}
                },
                InputMode::Name => match key.code {
                    KeyCode::Enter => { app.input_mode = InputMode::Target; },
                    KeyCode::Esc => { app.input_mode = InputMode::None; },
                    KeyCode::Char(c) => app.current_name.push(c),
                    KeyCode::Backspace => { app.current_name.pop(); },
                    _ => {},
                },
                InputMode::Target => match key.code {
                    KeyCode::Enter => { 
                        if let Some(i) = app.selected {
                            app.edit_dotfile(i, app.current_name.clone(), app.current_target.clone());
                        } else {
                            app.add_dotfile(app.current_name.clone(), app.current_target.clone()).ok();
                        }
                        app.input_mode = InputMode::None;
                        app.selected = None;
                    },
                    KeyCode::Esc => { app.input_mode = InputMode::None; },
                    KeyCode::Char(c) => app.current_target.push(c),
                    KeyCode::Backspace => { app.current_target.pop(); },
                    _ => {},
                },
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
