use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    event::{self, Event},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dd_dotstore::app::App;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::stdout;

fn main() -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    loop {
        terminal.draw(|f| app.draw(f))?;

        if let Event::Key(key) = event::read()?
            && app.handle_key(key)?
        {
            break;
        }
    }

    let _ = app.save();
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
