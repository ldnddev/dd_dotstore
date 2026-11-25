//! Chords shared by the window and the terminal.
//!
//! The window looks a key token up here. The terminal still matches keys in
//! `input::keys` so the existing Settings sheet, footer, and Ctrl+Q behavior
//! stay put. Settings is titled `Settings`. Its primary chords are `f1`,
//! `ctrl-/`, `f2`, and `ctrl-,`. About is `f3`. Quit is `ctrl-q`.
//!
//! `ctrl-/` and `ctrl-,` are window chords. The terminal opens Settings with
//! F1 and F2 only.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Settings,
    About,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Gui,
    Tui,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Kitty,
    Legacy,
    Tmux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingKind {
    Primary,
    Alias,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord<'a> {
    pub key: &'a str,
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
}

impl<'a> Chord<'a> {
    pub const fn plain(key: &'a str) -> Self {
        Self {
            key,
            control: false,
            alt: false,
            shift: false,
        }
    }

    pub const fn ctrl(key: &'a str) -> Self {
        Self {
            key,
            control: true,
            alt: false,
            shift: false,
        }
    }

    /// Parse a window token (`f1`, `ctrl-/`, `ctrl-,`, `ctrl-q`).
    pub fn parse(token: &'a str) -> Option<Self> {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }
        let mut parts: Vec<&str> = token.split('-').collect();
        let key = parts.pop()?;
        if key.is_empty() {
            return None;
        }
        let mut control = false;
        let mut alt = false;
        let mut shift = false;
        for part in parts {
            match part {
                "ctrl" => control = true,
                "alt" => alt = true,
                "shift" => shift = true,
                _ => return None,
            }
        }
        Some(Self {
            key,
            control,
            alt,
            shift,
        })
    }
}

pub fn chord_token(chord: Chord<'_>) -> String {
    let mut parts = Vec::new();
    if chord.control {
        parts.push("ctrl");
    }
    if chord.shift {
        parts.push("shift");
    }
    if chord.alt {
        parts.push("alt");
    }
    parts.push(chord.key);
    parts.join("-")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Binding {
    command: Command,
    chord: Chord<'static>,
    kind: BindingKind,
    gui: bool,
    tui: bool,
    vim_only: bool,
    outside_text: bool,
}

const BINDINGS: &[Binding] = &[
    Binding {
        command: Command::Settings,
        chord: Chord::plain("f1"),
        kind: BindingKind::Primary,
        gui: true,
        tui: true,
        vim_only: false,
        outside_text: false,
    },
    Binding {
        command: Command::Settings,
        chord: Chord::ctrl("/"),
        kind: BindingKind::Primary,
        gui: true,
        tui: true,
        vim_only: false,
        outside_text: false,
    },
    Binding {
        command: Command::Settings,
        chord: Chord::plain("f2"),
        kind: BindingKind::Primary,
        gui: true,
        tui: true,
        vim_only: false,
        outside_text: false,
    },
    Binding {
        command: Command::Settings,
        chord: Chord::ctrl(","),
        kind: BindingKind::Primary,
        gui: true,
        tui: true,
        vim_only: false,
        outside_text: false,
    },
    Binding {
        command: Command::About,
        chord: Chord::plain("f3"),
        kind: BindingKind::Primary,
        gui: true,
        tui: true,
        vim_only: false,
        outside_text: false,
    },
    Binding {
        command: Command::Quit,
        chord: Chord::ctrl("q"),
        kind: BindingKind::Primary,
        gui: true,
        tui: true,
        vim_only: false,
        outside_text: false,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: Command,
    pub title: &'static str,
    pub detail: &'static str,
    pub kbd: &'static str,
    /// Primary window tokens, in the order the sheet advertises them.
    pub chords: &'static [Chord<'static>],
    pub in_palette: bool,
}

const SPECS: &[CommandSpec] = &[
    CommandSpec {
        command: Command::Settings,
        title: "Settings",
        detail: "Themes, window, about, and help",
        kbd: "F1",
        chords: &[
            Chord::plain("f1"),
            Chord::ctrl("/"),
            Chord::plain("f2"),
            Chord::ctrl(","),
        ],
        in_palette: true,
    },
    CommandSpec {
        command: Command::About,
        title: "About",
        detail: "Credits and theme source",
        kbd: "F3",
        chords: &[Chord::plain("f3")],
        in_palette: true,
    },
    CommandSpec {
        command: Command::Quit,
        title: "Quit",
        detail: "Leave the window",
        kbd: "Ctrl+Q",
        chords: &[Chord::ctrl("q")],
        in_palette: true,
    },
];

/// One shortcut-sheet row. The first column is the window key. The second is the terminal key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetRow {
    pub section: &'static str,
    pub action: &'static str,
    pub gui: &'static str,
    pub tui: &'static str,
}

const SHEET: &[SheetRow] = &[
    SheetRow {
        section: "Global",
        action: "Settings",
        gui: "F1",
        tui: "F1",
    },
    SheetRow {
        section: "Global",
        action: "About",
        gui: "F3",
        tui: "F3",
    },
    SheetRow {
        section: "Global",
        action: "Quit",
        gui: "Ctrl+Q",
        tui: "Ctrl+Q",
    },
    SheetRow {
        section: "Tree",
        action: "Move",
        gui: "Up/Down",
        tui: "j/k",
    },
    SheetRow {
        section: "Tree",
        action: "Filter",
        gui: "/",
        tui: "/",
    },
    SheetRow {
        section: "Themes",
        action: "Preview",
        gui: "Up/Down",
        tui: "j/k",
    },
    SheetRow {
        section: "Themes",
        action: "Apply",
        gui: "Enter",
        tui: "Enter",
    },
    SheetRow {
        section: "Themes",
        action: "Colors",
        gui: "e",
        tui: "e",
    },
    SheetRow {
        section: "Themes",
        action: "Save",
        gui: "y/s",
        tui: "y/s",
    },
    SheetRow {
        section: "Themes",
        action: "Save target",
        gui: "t",
        tui: "t",
    },
    SheetRow {
        section: "Window",
        action: "Density",
        gui: "Enter",
        tui: "—",
    },
    SheetRow {
        section: "Window",
        action: "Vim keys",
        gui: "Enter",
        tui: "—",
    },
];

pub fn shortcut_sheet() -> &'static [SheetRow] {
    SHEET
}

pub fn shortcut_line(row: &SheetRow) -> String {
    format!("{} · {} · {}", row.action, row.gui, row.tui)
}

pub fn spec(command: Command) -> &'static CommandSpec {
    SPECS.iter()
        .find(|spec| spec.command == command)
        .expect("command spec")
}

pub fn lookup(
    token: &str,
    surface: Surface,
    mode: InputMode,
    vim: bool,
    in_text: bool,
) -> Option<Command> {
    let chord = Chord::parse(token)?;
    let mut found: Option<&Binding> = None;
    for binding in BINDINGS {
        if !binding_matches(binding, chord, surface, mode, vim, in_text) {
            continue;
        }
        if binding.kind == BindingKind::Primary {
            return Some(binding.command);
        }
        if found.is_none() {
            found = Some(binding);
        }
    }
    found.map(|binding| binding.command)
}

fn same_chord(left: Chord<'_>, right: Chord<'_>) -> bool {
    left.key == right.key
        && left.control == right.control
        && left.alt == right.alt
        && left.shift == right.shift
}

fn binding_matches(
    binding: &Binding,
    chord: Chord<'_>,
    surface: Surface,
    mode: InputMode,
    vim: bool,
    in_text: bool,
) -> bool {
    let surface_ok = match surface {
        Surface::Gui => binding.gui,
        Surface::Tui => binding.tui,
    };
    if !surface_ok || !same_chord(binding.chord, chord) {
        return false;
    }
    let mode_ok = match mode {
        InputMode::Kitty | InputMode::Legacy | InputMode::Tmux => true,
    };
    mode_ok && !(binding.vim_only && !vim) && !(binding.outside_text && in_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_primary_chords_and_first_help_row() {
        let spec = spec(Command::Settings);
        assert_eq!(spec.title, "Settings");
        assert_eq!(spec.kbd, "F1");
        let tokens: Vec<_> = spec.chords.iter().copied().map(chord_token).collect();
        assert_eq!(tokens, vec!["f1", "ctrl-/", "f2", "ctrl-,"]);
        for token in &tokens {
            let kind = BINDINGS
                .iter()
                .find(|binding| chord_token(binding.chord) == *token)
                .map(|binding| binding.kind);
            assert_eq!(kind, Some(BindingKind::Primary), "{token}");
            assert_eq!(
                lookup(token, Surface::Gui, InputMode::Kitty, false, true),
                Some(Command::Settings)
            );
        }
        assert_eq!(
            lookup("f3", Surface::Gui, InputMode::Kitty, true, false),
            Some(Command::About)
        );
        assert_eq!(
            lookup("ctrl-q", Surface::Gui, InputMode::Kitty, true, true),
            Some(Command::Quit)
        );
        assert_eq!(
            lookup("q", Surface::Gui, InputMode::Kitty, true, false),
            None
        );
        let line = shortcut_line(&shortcut_sheet()[0]);
        assert_eq!(line, "Settings · F1 · F1");
    }
}
