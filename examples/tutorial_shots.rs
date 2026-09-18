//! Capture named TUI frames for `docs/index.html`.
//!
//! Run via `./docs/capture.sh` (preferred) or:
//!
//! ```bash
//! cargo run --example tutorial_shots --release
//! ```
//!
//! This writes one self-contained HTML file per shot under
//! `docs/images/_frames/`. `docs/capture.sh` then screenshots those files
//! into `docs/images/*.png`.
//!
//! To add a shot: append a `capture(...)` call in `main`, document it in
//! `docs/README.md`, and reference the PNG from `docs/index.html`.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_dotstore::app::App;
use dd_dotstore::domain::NodeKind;
use dd_dotstore::tree::flatten_visible;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const COLS: u16 = 124;
const ROWS: u16 = 34;
const HEADER_QUOTE: &str = "Don't Fear the . (Dot) - Tame It.";

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let frames_dir = repo.join("docs/images/_frames");
    fs::create_dir_all(&frames_dir).context("create frames dir")?;
    for old in fs::read_dir(&frames_dir)? {
        let old = old?;
        let _ = fs::remove_file(old.path());
    }

    // Short, stable paths so dest columns stay readable in screenshots.
    let home = PathBuf::from("/tmp/dd-home");
    let project = PathBuf::from("/tmp/dd-dots");
    rebuild_fixture(&repo, &home, &project)?;

    // Isolate HOME / XDG so adopt + doctor never walk the real user tree,
    // and so dest paths in screenshots stay stable.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));
    }

    let mut app = App::new_with_root(&project).context("init app")?;
    app.state.header_copy = HEADER_QUOTE.to_string();
    expand_all(&mut app);
    if !app.state.nodes.is_empty() {
        app.state.list_state.select(Some(0));
    }

    let mut shots = Vec::new();

    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "01-main",
        "Main window: source tree, destinations, status icons",
    )?;

    press(&mut app, KeyCode::F(1))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "02-help",
        "F1 help modal (keys + mouse)",
    )?;
    press(&mut app, KeyCode::Esc)?;

    press(&mut app, KeyCode::Char('/'))?;
    type_text(&mut app, "nvim")?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "03-filter",
        "Inline source filter matching nvim",
    )?;
    press(&mut app, KeyCode::Esc)?;

    select_path(&mut app, ".bashrc");
    press(&mut app, KeyCode::Char('e'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "04-dest-browser",
        "Destination filesystem browser",
    )?;
    press(&mut app, KeyCode::Esc)?;

    select_path(&mut app, ".config/nvim");
    press(&mut app, KeyCode::Char('s'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "05-plan",
        "Plan apply dialog (Y to apply)",
    )?;
    press(&mut app, KeyCode::Esc)?;

    select_path(&mut app, ".gitconfig");
    press(&mut app, KeyCode::Char('s'))?;
    press(&mut app, KeyCode::Char('Y'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "06-overwrite",
        "Overwrite warning for a real file dest",
    )?;
    press(&mut app, KeyCode::Esc)?;

    press(&mut app, KeyCode::F(2))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "07-theme",
        "F2 live theme editor",
    )?;
    press(&mut app, KeyCode::Esc)?;

    press(&mut app, KeyCode::Char('I'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "08-ignore",
        "Ignore pattern editor",
    )?;
    press(&mut app, KeyCode::Esc)?;

    press(&mut app, KeyCode::Char('D'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "09-doctor",
        "Doctor: broken, planned, and orphan dests",
    )?;
    press(&mut app, KeyCode::Esc)?;

    press(&mut app, KeyCode::F(3))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "10-credits",
        "F3 credits + theme status",
    )?;
    press(&mut app, KeyCode::Esc)?;

    select_path(&mut app, ".zshrc");
    press(&mut app, KeyCode::Char('t'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "11-group",
        "Named group editor",
    )?;
    press(&mut app, KeyCode::Esc)?;

    press(&mut app, KeyCode::Char('A'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "12-adopt",
        "Reverse-import (adopt) existing HOME/XDG symlinks",
    )?;
    press(&mut app, KeyCode::Esc)?;

    // Checkboxes + toast. T selects the highlight's group.
    select_path(&mut app, ".zshrc");
    press(&mut app, KeyCode::Char('T'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "13-selected",
        "Multi-select, group highlight, and a success toast",
    )?;

    press(&mut app, KeyCode::Esc)?;
    app.state.toast = None;
    select_path(&mut app, "scripts/bootstrap.sh");
    press(&mut app, KeyCode::Char('m'))?;
    capture(
        &mut app,
        &frames_dir,
        &mut shots,
        "14-copy-mode",
        "LINK/COPY toggle on the highlighted row",
    )?;

    write_manifest(&frames_dir, &shots)?;
    println!(
        "Wrote {} HTML frames to {}",
        shots.len(),
        frames_dir.display()
    );
    for shot in &shots {
        println!("  {}.html  — {}", shot.id, shot.caption);
    }
    Ok(())
}

struct Shot {
    id: String,
    caption: String,
}

fn capture(
    app: &mut App,
    frames_dir: &Path,
    shots: &mut Vec<Shot>,
    id: &str,
    caption: &str,
) -> Result<()> {
    let backend = TestBackend::new(COLS, ROWS);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| app.draw(f))?;
    let html = render_html(terminal.backend().buffer(), id, caption);
    fs::write(frames_dir.join(format!("{id}.html")), html)?;
    shots.push(Shot {
        id: id.to_string(),
        caption: caption.to_string(),
    });
    Ok(())
}

fn press(app: &mut App, code: KeyCode) -> Result<bool> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(app: &mut App, text: &str) -> Result<()> {
    for c in text.chars() {
        press(app, KeyCode::Char(c))?;
    }
    Ok(())
}

fn select_path(app: &mut App, rel: &str) {
    let path = PathBuf::from(rel);
    if let Some(idx) = app.state.nodes.iter().position(|n| n.path == path) {
        app.state.list_state.select(Some(idx));
    } else {
        eprintln!("warning: path not visible in tree: {rel}");
    }
}

fn expand_all(app: &mut App) {
    fn rec(nodes: &mut [dd_dotstore::domain::Node]) {
        for node in nodes {
            if let NodeKind::Folder {
                expanded,
                children,
                ..
            } = &mut node.kind
            {
                *expanded = true;
                rec(children);
            }
        }
    }
    rec(&mut app.state.tree);
    flatten_visible(&mut app.state);
}

fn rebuild_fixture(repo: &Path, home: &Path, project: &Path) -> Result<()> {
    for path in [home, project] {
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
    }
    fs::create_dir_all(home.join(".config"))?;
    fs::create_dir_all(home.join(".local/bin"))?;
    fs::create_dir_all(home.join(".local/share"))?;
    fs::create_dir_all(home.join("Documents"))?;
    fs::create_dir_all(home.join("Pictures"))?;
    fs::create_dir_all(home.join("Downloads"))?;

    write_tree(
        project,
        &[
            (".zshrc", "export EDITOR=nvim\n"),
            (".bashrc", "alias ll='ls -la'\n"),
            (".gitconfig", "[user]\n\tname = You\n"),
            (
                ".ssh/config",
                "Host github.com\n  IdentityFile ~/.ssh/id_ed25519\n",
            ),
            (".config/nvim/init.lua", "-- nvim\n"),
            (".config/nvim/lua/plugins.lua", "-- plugins\n"),
            (
                ".config/alacritty/alacritty.toml",
                "[window]\nopacity = 0.95\n",
            ),
            (
                ".config/starship.toml",
                "[character]\nsuccess_symbol = '❯'\n",
            ),
            ("scripts/bootstrap.sh", "#!/bin/sh\necho hi\n"),
            ("README.md", "# my dotfiles\n"),
        ],
    )?;
    fs::copy(
        repo.join("dd_dotstore_theme.yml"),
        project.join("dd_dotstore_theme.yml"),
    )
    .context("copy theme into fixture")?;

    // Valid LINK: dest is a symlink pointing at the project file.
    symlink(project.join(".zshrc"), home.join(".zshrc"))?;
    // Broken LINK: dest exists as a regular file, not a symlink to the source.
    fs::write(home.join(".gitconfig"), "[user]\n\tname = stale\n")?;
    // Orphan (in HOME/XDG, not in config) for Adopt + Doctor.
    symlink(
        project.join(".config/starship.toml"),
        home.join(".config/starship.toml"),
    )?;
    symlink(project.join(".bashrc"), home.join(".bashrc"))?;

    let config = format!(
        r#"{{
  "symlinks": {{
    ".zshrc": "{home}/.zshrc",
    ".gitconfig": "{home}/.gitconfig",
    ".config/nvim": "{home}/.config/nvim"
  }},
  "modes": {{
    "README.md": "copy"
  }},
  "groups": {{
    ".zshrc": "shell",
    ".bashrc": "shell",
    ".config/nvim": "nvim"
  }},
  "history": [],
  "ignore_patterns": [
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".dd_dotstore.json",
    ".DS_Store",
    ".linked"
  ]
}}"#,
        home = home.display()
    );
    fs::write(project.join(".dd_dotstore.json"), config)?;
    Ok(())
}

fn write_tree(root: &Path, files: &[(&str, &str)]) -> Result<()> {
    for (rel, body) in files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, body)?;
    }
    Ok(())
}

fn write_manifest(frames_dir: &Path, shots: &[Shot]) -> Result<()> {
    let mut out = String::from("{\n  \"cols\": ");
    out.push_str(&COLS.to_string());
    out.push_str(",\n  \"rows\": ");
    out.push_str(&ROWS.to_string());
    out.push_str(",\n  \"shots\": [\n");
    for (i, shot) in shots.iter().enumerate() {
        out.push_str("    { \"id\": \"");
        out.push_str(&shot.id);
        out.push_str("\", \"caption\": \"");
        out.push_str(&json_escape(&shot.caption));
        out.push_str("\" }");
        if i + 1 != shots.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}\n");
    fs::write(frames_dir.join("manifest.json"), out)?;
    Ok(())
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_html(buffer: &ratatui::buffer::Buffer, id: &str, caption: &str) -> String {
    let body = render_pre(buffer);
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{id}</title>
<style>
  html, body {{
    margin: 0;
    background: #0b0d10;
    color: #f5f6f7;
  }}
  .shot {{
    display: inline-block;
    background: #0f1114;
    border: 1px solid #2a2d31;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 22px 60px rgba(0, 0, 0, 0.45);
  }}
  .titlebar {{
    height: 34px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    background: #1c1e21;
    color: #9ea3aa;
    font: 12px "JetBrainsMono Nerd Font", "JetBrains Mono", ui-monospace, monospace;
  }}
  .dots {{ display: flex; gap: 6px; }}
  .dot {{ width: 10px; height: 10px; border-radius: 50%; }}
  .term {{
    margin: 0;
    padding: 8px 10px 10px;
    font: 15px/20px "JetBrainsMono Nerd Font", "JetBrains Mono", ui-monospace, monospace;
    white-space: pre;
    background: #0f1114;
  }}
</style>
</head>
<body>
  <div class="shot" data-id="{id}" data-caption="{caption_esc}">
    <div class="titlebar">
      <span class="dots">
        <span class="dot" style="background:#e57373"></span>
        <span class="dot" style="background:#f5c469"></span>
        <span class="dot" style="background:#82e0aa"></span>
      </span>
      <span>dd_dotstore — {caption_esc}</span>
    </div>
    <pre class="term">{body}</pre>
  </div>
</body>
</html>
"##,
        caption_esc = html_escape(caption),
    )
}

fn render_pre(buffer: &ratatui::buffer::Buffer) -> String {
    let width = buffer.area.width;
    let height = buffer.area.height;
    let mut out = String::new();
    for y in 0..height {
        let mut x = 0u16;
        while x < width {
            let cell = &buffer[(x, y)];
            let fg = css_color(cell.fg, "#f5f6f7");
            let bg = css_color(cell.bg, "#0f1114");
            let bold = cell.modifier.contains(Modifier::BOLD);
            let dim = cell.modifier.contains(Modifier::DIM);
            let mut run = cell.symbol().to_string();
            let mut run_w = 1u16;
            while x + run_w < width {
                let next = &buffer[(x + run_w, y)];
                if css_color(next.fg, "#f5f6f7") != fg
                    || css_color(next.bg, "#0f1114") != bg
                    || next.modifier.contains(Modifier::BOLD) != bold
                    || next.modifier.contains(Modifier::DIM) != dim
                {
                    break;
                }
                run.push_str(next.symbol());
                run_w += 1;
            }
            let mut style = format!("color:{fg};background:{bg}");
            if bold {
                style.push_str(";font-weight:700");
            }
            if dim {
                style.push_str(";opacity:0.72");
            }
            out.push_str("<span style=\"");
            out.push_str(&style);
            out.push_str("\">");
            out.push_str(&html_escape(&run));
            out.push_str("</span>");
            x += run_w;
        }
        if y + 1 != height {
            out.push('\n');
        }
    }
    out
}

fn css_color(color: Color, fallback: &str) -> String {
    match color {
        Color::Reset => fallback.to_string(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Black => "#000000".into(),
        Color::Red => "#e57373".into(),
        Color::Green => "#82e0aa".into(),
        Color::Yellow => "#f5c469".into(),
        Color::Blue => "#64b4f5".into(),
        Color::Magenta => "#ffa087".into(),
        Color::Cyan => "#5dade2".into(),
        Color::Gray => "#9ea3aa".into(),
        Color::DarkGray => "#2a2d31".into(),
        Color::LightRed => "#ef9a9a".into(),
        Color::LightGreen => "#a9dfbf".into(),
        Color::LightYellow => "#f8d486".into(),
        Color::LightBlue => "#90caf9".into(),
        Color::LightMagenta => "#ffc1b3".into(),
        Color::LightCyan => "#85c1e9".into(),
        Color::White => "#f5f6f7".into(),
        Color::Indexed(i) => format!("#{i:02x}{i:02x}{i:02x}"),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
