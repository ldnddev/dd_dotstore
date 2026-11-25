//! Headless shell tests. They read `visible_text`, not a drawn frame.

use std::fs;
use std::path::Path;

use gpui::{AppContext, TestAppContext};

use crate::shell::{Shell, ShellOptions};

fn mount(cx: &mut TestAppContext, root: &Path, settings_file: &Path) -> gpui::Entity<Shell> {
    cx.update(gpui_component::init);
    let root = root.to_path_buf();
    let settings_file = settings_file.to_path_buf();
    cx.new(|cx| {
        Shell::open(
            ShellOptions {
                root: Some(root),
                settings_file,
                quit_runtime: false,
            },
            cx,
        )
    })
}

fn text(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext) -> String {
    shell.update(cx, |shell, _| shell.visible_text())
}

fn press(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext, key: &str) {
    shell.update(cx, |shell, _| shell.handle_key(key));
}

fn quit(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext) -> bool {
    shell.update(cx, |shell, _| shell.quit)
}

fn project(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(name);
    fs::create_dir(&root).unwrap();
    dir
}

fn open_color_editor(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext) {
    press(shell, cx, "f1");
    if text(shell, cx).contains("theme-list:flow") {
        press(shell, cx, "e");
    }
}

fn hex_line(body: &str) -> String {
    body.lines()
        .find_map(|line| line.strip_prefix("Hex: ").map(str::to_string))
        .unwrap_or_default()
}

fn nudge_hex(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext) -> (String, String) {
    open_color_editor(shell, cx);
    let before = hex_line(&text(shell, cx));
    press(shell, cx, "right");
    let mut after = hex_line(&text(shell, cx));
    if after == before {
        press(shell, cx, "left");
        after = hex_line(&text(shell, cx));
    }
    assert_ne!(before, after, "color nudge did not change the hex");
    (before, after)
}

#[gpui::test]
fn body_lists_a_root_dotfile(cx: &mut TestAppContext) {
    let dir = project("dots");
    let root = dir.path().join("dots");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    let body = text(&shell, cx);
    assert!(body.contains(".bashrc"), "{body}");
    assert!(body.contains("file"), "{body}");
    assert!(body.contains("F1: Settings"), "{body}");
    assert!(body.contains("Ctrl+Q: Quit"), "{body}");
    assert!(!body.contains("settings-panel:right:70"), "{body}");
}

#[gpui::test]
fn up_down_move_the_tree_selection(cx: &mut TestAppContext) {
    let dir = project("two");
    let root = dir.path().join("two");
    fs::write(root.join(".aaa"), "a\n").unwrap();
    fs::write(root.join(".bbb"), "b\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    let body = text(&shell, cx);
    assert!(body.contains("> .aaa file"), "{body}");
    assert!(body.contains("  .bbb file"), "{body}");
    press(&shell, cx, "down");
    let body = text(&shell, cx);
    assert!(body.contains("> .bbb file"), "{body}");
    assert!(body.contains("  .aaa file"), "{body}");
    press(&shell, cx, "up");
    assert!(text(&shell, cx).contains("> .aaa file"));
    press(&shell, cx, "j");
    assert!(text(&shell, cx).contains("> .bbb file"));
}

#[gpui::test]
fn vim_off_leaves_j_alone_and_arrows_still_move(cx: &mut TestAppContext) {
    let dir = project("novim");
    let root = dir.path().join("novim");
    fs::write(root.join(".aaa"), "a\n").unwrap();
    fs::write(root.join(".bbb"), "b\n").unwrap();
    let settings = dir.path().join("gui.json");
    fs::write(
        &settings,
        r#"{"density":"comfortable","vim_keys":false,"reduced_motion":false}"#,
    )
    .unwrap();
    let shell = mount(cx, &root, &settings);
    assert!(text(&shell, cx).contains("> .aaa file"));
    press(&shell, cx, "j");
    assert!(text(&shell, cx).contains("> .aaa file"));
    press(&shell, cx, "down");
    assert!(text(&shell, cx).contains("> .bbb file"));
}

#[gpui::test]
fn filter_accepts_q_and_bare_q_does_not_quit(cx: &mut TestAppContext) {
    let dir = project("filter");
    let root = dir.path().join("filter");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    fs::write(root.join(".quiet"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "q");
    assert!(!quit(&shell, cx));
    assert!(!text(&shell, cx).contains("filter:q"));
    press(&shell, cx, "/");
    press(&shell, cx, "q");
    let body = text(&shell, cx);
    assert!(body.contains("filter:q"), "{body}");
    assert!(body.contains(".quiet"), "{body}");
    assert!(!body.contains(".bashrc"), "{body}");
    assert!(!quit(&shell, cx));
}

#[gpui::test]
fn settings_sheet_pages_and_second_f1(cx: &mut TestAppContext) {
    let dir = project("sheet");
    let root = dir.path().join("sheet");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));

    press(&shell, cx, "f1");
    let body = text(&shell, cx);
    assert!(body.contains("settings-panel:right:70"), "{body}");
    assert!(
        body.contains("settings-nav:Themes,Window,About,Help"),
        "{body}"
    );
    assert!(body.contains("settings-page:Themes"), "{body}");
    assert!(
        body.contains("theme-list:flow") || body.contains("Theme color editor"),
        "{body}"
    );
    assert!(body.contains("F1: Settings"), "{body}");
    assert!(body.contains("Esc: Close"), "{body}");
    assert!(body.contains("Ctrl+Q: Quit"), "{body}");

    press(&shell, cx, "tab");
    let body = text(&shell, cx);
    assert!(body.contains("settings-page:Window"), "{body}");
    assert!(body.contains("Density"), "{body}");
    assert!(body.contains("Vim"), "{body}");
    assert!(
        body.contains("Comfortable") || body.contains("Compact"),
        "{body}"
    );

    press(&shell, cx, "tab");
    let body = text(&shell, cx);
    assert!(body.contains("settings-page:About"), "{body}");
    assert!(body.contains("GNU Stow"), "{body}");
    assert!(body.contains("Ratatui + Crossterm"), "{body}");

    press(&shell, cx, "tab");
    let body = text(&shell, cx);
    assert!(body.contains("settings-page:Help"), "{body}");
    assert!(body.contains("Settings · F1 · F1"), "{body}");

    press(&shell, cx, "f1");
    assert!(!text(&shell, cx).contains("settings-panel:right:70"));
}

#[gpui::test]
fn f2_ctrl_slash_and_ctrl_comma_toggle_the_same_sheet(cx: &mut TestAppContext) {
    let dir = project("chords");
    let root = dir.path().join("chords");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    for key in ["f2", "ctrl-/", "ctrl-,"] {
        press(&shell, cx, key);
        assert!(
            text(&shell, cx).contains("settings-panel:right:70"),
            "{key}"
        );
        assert!(text(&shell, cx).contains("settings-page:Themes"), "{key}");
        press(&shell, cx, key);
        assert!(
            !text(&shell, cx).contains("settings-panel:right:70"),
            "{key}"
        );
    }
}

#[gpui::test]
fn f3_opens_about_and_switches_without_keeping_a_preview(cx: &mut TestAppContext) {
    let dir = project("about");
    let root = dir.path().join("about");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));

    press(&shell, cx, "f3");
    let body = text(&shell, cx);
    assert!(body.contains("settings-page:About"), "{body}");
    assert!(body.contains("settings-panel:right:70"), "{body}");
    assert!(body.contains("Credits"), "{body}");
    press(&shell, cx, "f1");
    assert!(!text(&shell, cx).contains("settings-panel:right:70"));

    let (original, _) = nudge_hex(&shell, cx);
    press(&shell, cx, "f3");
    let body = text(&shell, cx);
    assert!(body.contains("settings-page:About"), "{body}");
    assert!(body.contains("settings-panel:right:70"), "{body}");
    press(&shell, cx, "escape");
    assert!(!text(&shell, cx).contains("settings-panel:right:70"));

    open_color_editor(&shell, cx);
    assert_eq!(hex_line(&text(&shell, cx)), original);
}

#[gpui::test]
fn escape_reverts_the_palette_snapshot(cx: &mut TestAppContext) {
    let dir = project("revert");
    let root = dir.path().join("revert");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    let (original, _) = nudge_hex(&shell, cx);
    press(&shell, cx, "escape");
    if text(&shell, cx).contains("settings-panel:right:70") {
        assert!(text(&shell, cx).contains("theme-list:flow"));
        press(&shell, cx, "escape");
    }
    assert!(!text(&shell, cx).contains("settings-panel:right:70"));
    open_color_editor(&shell, cx);
    assert_eq!(hex_line(&text(&shell, cx)), original);
}

#[gpui::test]
fn ctrl_q_quits_while_the_sheet_is_open(cx: &mut TestAppContext) {
    let dir = project("quit");
    let root = dir.path().join("quit");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "f1");
    press(&shell, cx, "ctrl-q");
    assert!(quit(&shell, cx));
    assert!(text(&shell, cx).contains("settings-panel:right:70"));
}

#[gpui::test]
fn gui_json_round_trip_does_not_touch_the_project_config(cx: &mut TestAppContext) {
    let dir = project("prefs");
    let root = dir.path().join("prefs");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let config = root.join(".dd_dotstore.json");
    let sentinel = b"{\"symlinks\":{}}\n";
    fs::write(&config, sentinel).unwrap();
    let settings = dir.path().join("gui.json");
    fs::write(
        &settings,
        "{\n  \"density\": \"compact\",\n  \"vim_keys\": true,\n  \"reduced_motion\": false\n}\n",
    )
    .unwrap();

    let shell = mount(cx, &root, &settings);
    let body = text(&shell, cx);
    assert!(body.contains("density:compact"), "{body}");
    press(&shell, cx, "f1");
    press(&shell, cx, "tab");
    assert!(text(&shell, cx).contains("settings-page:Window"));
    assert!(text(&shell, cx).contains("Density Compact"));
    press(&shell, cx, "enter");
    let saved = fs::read_to_string(&settings).unwrap();
    assert!(saved.contains("comfortable"), "{saved}");
    assert!(saved.contains("vim_keys"), "{saved}");
    assert_eq!(fs::read(&config).unwrap(), sentinel);
    assert!(!root.join("dd_dotstore_theme.yml").exists());
    assert!(!saved.contains("base_background"), "{saved}");
}
