use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    event::{self, Event},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dd_dotstore::app::App;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, Write, stdout};
use std::path::PathBuf;

fn main() -> Result<()> {
    match parse_cli(std::env::args().skip(1)) {
        Ok(CliAction::PrintHelp) => {
            print_help(&mut io::stdout())?;
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

fn run_app(project_root: Option<PathBuf>) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = if let Some(root) = project_root {
        App::new_with_root(&root)?
    } else {
        App::new()?
    };

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
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(Option<PathBuf>),
    PrintHelp,
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
}
