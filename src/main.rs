use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dd_dotstore::app::App;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, Write, stdout};
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<()> {
    match parse_cli(std::env::args().skip(1)) {
        Ok(CliAction::PrintHelp) => {
            print_help(&mut io::stdout())?;
            return Ok(());
        }
        Ok(CliAction::PrintVersion) => {
            writeln!(io::stdout(), "dd_dotstore {}", env!("CARGO_PKG_VERSION"))?;
            return Ok(());
        }
        Err(err) => {
            let mut stderr = io::stderr();
            writeln!(stderr, "Error: {err}")?;
            writeln!(stderr)?;
            print_help(&mut stderr)?;
            std::process::exit(2);
        }
        Ok(CliAction::Run(project_root)) => run_app(project_root)?,
    }
    Ok(())
}

struct TerminalGuard {
    raw: bool,
    alt: bool,
    mouse: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        if self.mouse {
            let _ = out.execute(DisableMouseCapture);
        }
        if self.alt {
            let _ = out.execute(LeaveAlternateScreen);
        }
        if self.raw {
            let _ = disable_raw_mode();
        }
    }
}

fn run_app(project_root: Option<PathBuf>) -> Result<()> {
    let mut guard = TerminalGuard {
        raw: false,
        alt: false,
        mouse: false,
    };
    enable_raw_mode()?;
    guard.raw = true;
    stdout().execute(EnterAlternateScreen)?;
    guard.alt = true;
    guard.mouse = stdout().execute(EnableMouseCapture).is_ok();

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = if let Some(root) = project_root {
        App::new_with_root(&root)?
    } else {
        App::new()?
    };

    loop {
        app.tick();
        terminal.draw(|f| app.draw(f))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if app.handle_key(key)? {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    if app.handle_mouse(mouse)? {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    // Next draw iteration will handle the new size
                }
                _ => {}
            }
        }
    }

    let config_path = app.state.config_path.clone();
    let save_res = app.state.persist_now_if_dirty();
    drop(terminal);
    drop(guard);
    if let Err(err) = save_res {
        eprintln!("Failed to save {}: {err}", config_path.display());
        std::process::exit(1);
    }
    Ok(())
}

fn print_help(w: &mut impl Write) -> Result<()> {
    writeln!(w, "dd_dotstore - dotfile symlink manager")?;
    writeln!(w)?;
    writeln!(w, "Usage:")?;
    writeln!(w, "  dd_dotstore [--root <path>] [path]")?;
    writeln!(w)?;
    writeln!(w, "Options:")?;
    writeln!(w, "  --root <path>   Set project root explicitly")?;
    writeln!(w, "  --root=<path>   Same as above")?;
    writeln!(w, "  -h, --help      Show this help")?;
    writeln!(w, "  -V, --version   Print version and exit")?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(Option<PathBuf>),
    PrintHelp,
    PrintVersion,
}

fn parse_cli<I>(args: I) -> std::result::Result<CliAction, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let mut positional: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Ok(CliAction::PrintHelp);
        }
        if arg == "--version" || arg == "-V" {
            return Ok(CliAction::PrintVersion);
        }
        if arg == "--root" {
            if let Some(path) = args.next() {
                if positional.is_some() {
                    return Err("Provide either --root or positional path, not both".to_string());
                }
                return Ok(CliAction::Run(Some(PathBuf::from(path))));
            }
            return Err("Missing value for --root".to_string());
        }
        if let Some(value) = arg.strip_prefix("--root=") {
            if positional.is_some() {
                return Err("Provide either --root or positional path, not both".to_string());
            }
            return Ok(CliAction::Run(Some(PathBuf::from(value))));
        }
        if arg.starts_with('-') {
            return Err(format!("Unknown option: {arg}"));
        }
        if positional.is_none() {
            positional = Some(PathBuf::from(arg));
        } else {
            return Err("Only one positional path is supported".to_string());
        }
    }

    Ok(CliAction::Run(positional))
}

#[cfg(test)]
mod tests {
    use super::{CliAction, parse_cli};
    use std::path::PathBuf;

    #[test]
    fn parse_root_from_flag_or_positional() {
        let cli = parse_cli(vec!["--root".to_string(), "/tmp/dots".to_string()]).expect("parse");
        assert_eq!(cli, CliAction::Run(Some(PathBuf::from("/tmp/dots"))));

        let cli = parse_cli(vec!["--root=/tmp/x".to_string()]).expect("parse");
        assert_eq!(cli, CliAction::Run(Some(PathBuf::from("/tmp/x"))));

        let cli = parse_cli(vec!["/tmp/positional".to_string()]).expect("parse");
        assert_eq!(cli, CliAction::Run(Some(PathBuf::from("/tmp/positional"))));
    }

    #[test]
    fn parse_help_and_errors() {
        let cli = parse_cli(vec!["--help".to_string()]).expect("help");
        assert_eq!(cli, CliAction::PrintHelp);

        let err = parse_cli(vec!["--root".to_string()]).expect_err("missing value error");
        assert!(err.contains("Missing value"));

        let err = parse_cli(vec!["--bogus".to_string()]).expect_err("unknown option error");
        assert!(err.contains("Unknown option"));
    }

    #[test]
    fn parse_version_flag() {
        let cli = parse_cli(vec!["--version".to_string()]).expect("version");
        assert_eq!(cli, CliAction::PrintVersion);

        let cli = parse_cli(vec!["-V".to_string()]).expect("short version");
        assert_eq!(cli, CliAction::PrintVersion);
    }
}
