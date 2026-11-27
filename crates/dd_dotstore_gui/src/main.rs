//! GPUI window for dd_dotstore. The terminal binary stays the default build.

mod present;
mod settings;
mod shell;
mod windows;

#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::PathBuf;

use gpui::{App, AppContext, Focusable};
use gpui_component::Root;
use shell::{Shell, ShellOptions};

fn main() {
    match parse_cli(std::env::args().skip(1)) {
        Ok(Cli::Help) => {
            let _ = print_help(&mut io::stdout());
        }
        Ok(Cli::Version) => {
            println!("dd_dotstore-gui {}", env!("CARGO_PKG_VERSION"));
        }
        Err(err) => {
            let mut stderr = io::stderr();
            let _ = writeln!(stderr, "Error: {err}");
            let _ = writeln!(stderr);
            let _ = print_help(&mut stderr);
            std::process::exit(2);
        }
        Ok(Cli::Run(root)) => run(root),
    }
}

fn run(root: Option<PathBuf>) {
    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        let root = root.clone();
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                let bounds = windows::centered_bounds(cx);
                cx.open_window(windows::main_window_options(bounds), |window, cx| {
                    let view = cx.new(|cx| {
                        let shell = Shell::open(ShellOptions::production(root), cx);
                        window.focus(&shell.focus_handle(cx), cx);
                        shell
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("open window");
            });
        })
        .detach();
    });
}

fn print_help(w: &mut impl Write) -> io::Result<()> {
    writeln!(w, "dd_dotstore-gui - dotfile symlink manager")?;
    writeln!(w)?;
    writeln!(w, "Usage:")?;
    writeln!(w, "  dd_dotstore-gui [--root <path>] [path]")?;
    writeln!(w)?;
    writeln!(w, "Options:")?;
    writeln!(w, "  --root <path>   Set project root explicitly")?;
    writeln!(w, "  --root=<path>   Same as above")?;
    writeln!(w, "  -h, --help      Show this help")?;
    writeln!(w, "  -V, --version   Print version and exit")?;
    writeln!(w)?;
    writeln!(w, "The window needs Vulkan and Wayland or X11.")?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Run(Option<PathBuf>),
    Help,
    Version,
}

fn parse_cli<I>(args: I) -> Result<Cli, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let mut positional: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Ok(Cli::Help);
        }
        if arg == "--version" || arg == "-V" {
            return Ok(Cli::Version);
        }
        if arg == "--root" {
            if let Some(path) = args.next() {
                if positional.is_some() {
                    return Err("Provide either --root or positional path, not both".to_string());
                }
                return Ok(Cli::Run(Some(PathBuf::from(path))));
            }
            return Err("Missing value for --root".to_string());
        }
        if let Some(value) = arg.strip_prefix("--root=") {
            if positional.is_some() {
                return Err("Provide either --root or positional path, not both".to_string());
            }
            return Ok(Cli::Run(Some(PathBuf::from(value))));
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

    Ok(Cli::Run(positional))
}

#[cfg(test)]
mod cli_tests {
    use super::{Cli, parse_cli};
    use std::path::PathBuf;

    #[test]
    fn help_and_version_do_not_become_a_root() {
        assert_eq!(parse_cli(["--help".to_string()]).unwrap(), Cli::Help);
        assert_eq!(parse_cli(["-V".to_string()]).unwrap(), Cli::Version);
    }

    #[test]
    fn root_flag_and_one_positional() {
        let cli = parse_cli(["--root".to_string(), "/tmp/dots".to_string()]).unwrap();
        assert_eq!(cli, Cli::Run(Some(PathBuf::from("/tmp/dots"))));
        let cli = parse_cli(["--root=/tmp/x".to_string()]).unwrap();
        assert_eq!(cli, Cli::Run(Some(PathBuf::from("/tmp/x"))));
        let cli = parse_cli(["/tmp/pos".to_string()]).unwrap();
        assert_eq!(cli, Cli::Run(Some(PathBuf::from("/tmp/pos"))));
        assert!(parse_cli(["a".to_string(), "b".to_string()]).is_err());
        assert!(parse_cli(["--nope".to_string()]).is_err());
    }
}
