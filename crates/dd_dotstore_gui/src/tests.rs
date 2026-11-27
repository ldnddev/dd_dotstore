//! Headless shell tests. They read `visible_text`, not a drawn frame.

use std::fs;
use std::path::Path;

use dd_dotstore::domain::Modal;
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
    assert!(body.contains("> [ ] · [LINK] .aaa file"), "{body}");
    assert!(body.contains("  [ ] · [LINK] .bbb file"), "{body}");
    press(&shell, cx, "down");
    let body = text(&shell, cx);
    assert!(body.contains("> [ ] · [LINK] .bbb file"), "{body}");
    assert!(body.contains("  [ ] · [LINK] .aaa file"), "{body}");
    press(&shell, cx, "up");
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .aaa file"));
    press(&shell, cx, "j");
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .bbb file"));
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
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .aaa file"));
    press(&shell, cx, "j");
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .aaa file"));
    press(&shell, cx, "down");
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .bbb file"));
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

fn retarget_browser(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext, dir: &Path) {
    let dir = dir.to_path_buf();
    shell.update(cx, |shell, _| {
        let app = shell.app.as_mut().expect("app");
        let Modal::EditDest { browser, .. } = app.state.modal.as_mut().expect("browser") else {
            panic!("expected destination browser");
        };
        browser.current = dir;
        browser.filter.clear();
        browser.selected = 0;
        browser.refresh_entries();
    });
}

#[gpui::test]
fn source_row_shows_mode_status_and_suggestion(cx: &mut TestAppContext) {
    let dir = project("suggest");
    let root = dir.path().join("suggest");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    let body = text(&shell, cx);
    assert!(body.contains("> [ ] · [LINK] .bashrc file"), "{body}");
    assert!(body.contains("suggested (not saved)"), "{body}");
    assert!(body.contains("destinations:0"), "{body}");
    assert!(body.starts_with("dd_dotstore\nquote:"), "{body}");
    assert!(body.contains("F1: Settings"), "{body}");
    assert!(body.contains("Space: Select"), "{body}");
}

#[gpui::test]
fn space_selects_m_toggles_copy_and_escape_clears(cx: &mut TestAppContext) {
    let dir = project("mode");
    let root = dir.path().join("mode");
    fs::write(root.join(".bashrc"), "x\n").unwrap();
    fs::write(root.join(".zshrc"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "space");
    assert!(text(&shell, cx).contains("> [✓] · [LINK] .bashrc file"));
    press(&shell, cx, "m");
    assert!(text(&shell, cx).contains("> [✓] · [COPY] .bashrc file"));
    press(&shell, cx, "shift-m");
    assert!(text(&shell, cx).contains("> [✓] · [LINK] .bashrc file"));
    press(&shell, cx, "escape");
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .bashrc file"));
    press(&shell, cx, "down");
    press(&shell, cx, "ctrl-g");
    assert!(text(&shell, cx).contains("> [ ] · [LINK] .bashrc file"));
}

#[gpui::test]
fn right_expands_a_folder_and_left_collapses_it(cx: &mut TestAppContext) {
    let dir = project("tree");
    let root = dir.path().join("tree");
    fs::create_dir(root.join("nvim")).unwrap();
    fs::write(root.join("nvim").join("init.lua"), "x\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    assert!(
        !text(&shell, cx).contains("init.lua"),
        "{}",
        text(&shell, cx)
    );
    press(&shell, cx, "right");
    assert!(text(&shell, cx).contains("init.lua"));
    press(&shell, cx, "left");
    assert!(!text(&shell, cx).contains("init.lua"));
    shell.update(cx, |shell, _| shell.click_twist(0));
    assert!(text(&shell, cx).contains("init.lua"));
}

#[gpui::test]
fn assign_plan_apply_and_undo_stay_inside_the_project(cx: &mut TestAppContext) {
    let dir = project("apply");
    let root = dir.path().join("apply");
    fs::write(root.join(".bashrc"), "export A=1\n").unwrap();
    let dest_dir = dir.path().join("linked");
    fs::create_dir(&dest_dir).unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));

    press(&shell, cx, "e");
    assert!(text(&shell, cx).contains("modal:dest"));
    retarget_browser(&shell, cx, &dest_dir);
    press(&shell, cx, "ctrl-s");
    let body = text(&shell, cx);
    assert!(!body.contains("modal:dest"), "{body}");
    assert!(body.contains("destinations:1"), "{body}");
    assert!(body.contains("planned"), "{body}");
    assert!(body.contains("○"), "{body}");

    press(&shell, cx, "s");
    let body = text(&shell, cx);
    assert!(body.contains("modal:plan"), "{body}");
    assert!(body.contains("Plan: Apply"), "{body}");
    press(&shell, cx, "y");
    let link = dest_dir.join(".bashrc");
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(text(&shell, cx).contains("linked"));
    assert!(!text(&shell, cx).contains("modal:plan"));

    press(&shell, cx, "u");
    assert!(!link.exists());
    assert!(text(&shell, cx).contains("planned") || text(&shell, cx).contains("suggested"));
}

#[gpui::test]
fn copy_apply_writes_a_file_and_remove_deletes_it(cx: &mut TestAppContext) {
    let dir = project("copy");
    let root = dir.path().join("copy");
    fs::write(root.join(".profile"), "echo hi\n").unwrap();
    let dest_dir = dir.path().join("linked");
    fs::create_dir(&dest_dir).unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "m");
    press(&shell, cx, "e");
    retarget_browser(&shell, cx, &dest_dir);
    press(&shell, cx, "ctrl-s");
    press(&shell, cx, "s");
    press(&shell, cx, "y");
    let dest = dest_dir.join(".profile");
    let meta = dest.symlink_metadata().unwrap();
    assert!(meta.is_file());
    assert!(!meta.file_type().is_symlink());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "echo hi\n");
    press(&shell, cx, "x");
    assert!(text(&shell, cx).contains("Plan: Remove"));
    press(&shell, cx, "y");
    assert!(!dest.exists());
}

#[gpui::test]
fn overwrite_dialog_blocks_a_real_file_until_confirmed(cx: &mut TestAppContext) {
    let dir = project("over");
    let root = dir.path().join("over");
    fs::write(root.join(".profile"), "new\n").unwrap();
    let dest_dir = dir.path().join("linked");
    fs::create_dir(&dest_dir).unwrap();
    fs::write(dest_dir.join(".profile"), "old\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "m");
    press(&shell, cx, "e");
    retarget_browser(&shell, cx, &dest_dir);
    press(&shell, cx, "ctrl-s");
    press(&shell, cx, "s");
    press(&shell, cx, "y");
    let body = text(&shell, cx);
    assert!(body.contains("modal:overwrite"), "{body}");
    assert_eq!(
        fs::read_to_string(dest_dir.join(".profile")).unwrap(),
        "old\n"
    );
    press(&shell, cx, "n");
    assert!(!text(&shell, cx).contains("modal:overwrite"));
    assert_eq!(
        fs::read_to_string(dest_dir.join(".profile")).unwrap(),
        "old\n"
    );
    press(&shell, cx, "s");
    press(&shell, cx, "y");
    press(&shell, cx, "y");
    assert_eq!(
        fs::read_to_string(dest_dir.join(".profile")).unwrap(),
        "new\n"
    );
}

#[gpui::test]
fn group_ignore_and_reload_round_trip_the_map(cx: &mut TestAppContext) {
    let dir = project("map");
    let root = dir.path().join("map");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    fs::write(root.join(".zshrc"), "b\n").unwrap();
    fs::write(root.join("skipme"), "c\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));

    press(&shell, cx, "t");
    assert!(text(&shell, cx).contains("modal:group"));
    for key in ["s", "h", "e", "l", "l"] {
        press(&shell, cx, key);
    }
    press(&shell, cx, "enter");
    let body = text(&shell, cx);
    assert!(body.contains("·shell"), "{body}");
    assert!(body.contains("toast:Success:"), "{body}");

    press(&shell, cx, "shift-t");
    assert!(text(&shell, cx).contains("[✓] · [LINK] .bashrc"));

    press(&shell, cx, "escape");
    press(&shell, cx, "shift-i");
    assert!(text(&shell, cx).contains("modal:ignore"));
    for key in ["s", "k", "i", "p", "m", "e"] {
        press(&shell, cx, key);
    }
    press(&shell, cx, "enter");
    press(&shell, cx, "escape");
    assert!(!text(&shell, cx).contains("skipme"));
    let saved = fs::read_to_string(root.join(".dd_dotstore.json")).unwrap();
    assert!(saved.contains("shell"), "{saved}");
    assert!(saved.contains("skipme"), "{saved}");
    assert!(
        saved.contains("\"history\": []") || saved.contains("\"history\":[]"),
        "{saved}"
    );
}

#[gpui::test]
fn export_and_import_use_the_forwarded_dialog(cx: &mut TestAppContext) {
    let dir = project("xfer");
    let root = dir.path().join("xfer");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "t");
    press(&shell, cx, "n");
    press(&shell, cx, "enter");

    let export_path = dir.path().join("out.json");
    shell.update(cx, |shell, _| {
        let app = shell.app.as_mut().unwrap();
        app.state.modal = Some(Modal::ExportPicker {
            dir: dir.path().to_path_buf(),
            filename: "out".to_string(),
        });
    });
    press(&shell, cx, "enter");
    assert!(export_path.is_file(), "export missing");
    let exported = fs::read_to_string(&export_path).unwrap();
    assert!(exported.contains(".bashrc"), "{exported}");

    shell.update(cx, |shell, _| {
        let app = shell.app.as_mut().unwrap();
        if let Some(node) =
            dd_dotstore::tree::find_mut_node(&mut app.state.tree, Path::new(".bashrc"))
        {
            node.group = None;
        }
        dd_dotstore::tree::flatten_visible(&mut app.state);
        app.state.modal = Some(Modal::ImportPicker {
            files: vec![export_path.clone()],
            selected: 0,
        });
    });
    press(&shell, cx, "enter");
    let body = text(&shell, cx);
    assert!(body.contains("·n"), "{body}");
    assert!(body.contains("toast:Success:"), "{body}");
}

#[gpui::test]
fn doctor_reports_a_planned_destination(cx: &mut TestAppContext) {
    let dir = project("doc");
    let root = dir.path().join("doc");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    let dest_dir = dir.path().join("missing-dest");
    fs::create_dir(&dest_dir).unwrap();
    press(&shell, cx, "e");
    retarget_browser(&shell, cx, &dest_dir);
    press(&shell, cx, "ctrl-s");
    press(&shell, cx, "shift-d");
    let body = text(&shell, cx);
    assert!(
        body.contains("modal:doctor") && body.contains("planned"),
        "{body}"
    );
    press(&shell, cx, "enter");
    assert!(!text(&shell, cx).contains("modal:doctor"));
}

#[gpui::test]
fn adopt_key_reports_a_result_without_writing(cx: &mut TestAppContext) {
    let dir = project("adopt");
    let root = dir.path().join("adopt");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "shift-a");
    let body = text(&shell, cx);
    assert!(
        body.contains("modal:adopt") || body.contains("No project symlinks"),
        "{body}"
    );
    if body.contains("modal:adopt") {
        press(&shell, cx, "escape");
        assert!(!text(&shell, cx).contains("modal:adopt"));
    }
}

#[gpui::test]
fn settings_stays_available_over_a_plan(cx: &mut TestAppContext) {
    let dir = project("over-sheet");
    let root = dir.path().join("over-sheet");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "s");
    assert!(text(&shell, cx).contains("modal:plan"));
    press(&shell, cx, "f1");
    let body = text(&shell, cx);
    assert!(body.contains("modal:plan"), "{body}");
    assert!(body.contains("settings-panel:right:70"), "{body}");
    press(&shell, cx, "escape");
    let body = text(&shell, cx);
    assert!(!body.contains("modal:plan"), "{body}");
    assert!(body.contains("settings-panel:right:70"), "{body}");
}

#[gpui::test]
fn help_lists_the_map_commands(cx: &mut TestAppContext) {
    let dir = project("helpkeys");
    let root = dir.path().join("helpkeys");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "f1");
    press(&shell, cx, "tab");
    press(&shell, cx, "tab");
    press(&shell, cx, "tab");
    let body = text(&shell, cx);
    assert!(body.contains("settings-page:Help"), "{body}");
    assert!(body.contains("Apply · s · s"), "{body}");
    assert!(body.contains("Doctor · D · D"), "{body}");
    assert!(body.contains("Settings · F1 · F1"), "{body}");
}

#[gpui::test]
fn checkbox_click_selects_the_row(cx: &mut TestAppContext) {
    let dir = project("click");
    let root = dir.path().join("click");
    fs::write(root.join(".aaa"), "a\n").unwrap();
    fs::write(root.join(".bbb"), "b\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    shell.update(cx, |shell, _| shell.click_checkbox(1));
    let body = text(&shell, cx);
    assert!(body.contains("> [✓] · [LINK] .bbb file"), "{body}");
    shell.update(cx, |shell, _| shell.click_source(0, true, 1));
    let body = text(&shell, cx);
    assert!(body.contains("[✓] · [LINK] .aaa file"), "{body}");
    assert!(body.contains("[✓] · [LINK] .bbb file"), "{body}");
}

#[gpui::test]
fn destination_click_jumps_to_a_collapsed_source(cx: &mut TestAppContext) {
    let dir = project("jump");
    let root = dir.path().join("jump");
    fs::create_dir(root.join("nvim")).unwrap();
    fs::write(root.join("nvim").join("init.lua"), "x\n").unwrap();
    let dest_dir = dir.path().join("linked");
    fs::create_dir(&dest_dir).unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "right");
    press(&shell, cx, "down");
    press(&shell, cx, "e");
    retarget_browser(&shell, cx, &dest_dir);
    press(&shell, cx, "ctrl-s");
    press(&shell, cx, "ctrl-g");
    press(&shell, cx, "left");
    let body = text(&shell, cx);
    let source = body.split("destinations:").next().unwrap_or(&body);
    assert!(!source.contains("init.lua"), "{body}");
    assert!(body.contains("destinations:1"), "{body}");
    shell.update(cx, |shell, _| shell.click_destination(0));
    let body = text(&shell, cx);
    let source = body.split("destinations:").next().unwrap_or(&body);
    assert!(
        source.contains("> [ ] ○ [LINK]") && source.contains("init.lua file"),
        "{body}"
    );
}

#[gpui::test]
fn dialog_click_moves_the_row_and_double_click_imports(cx: &mut TestAppContext) {
    let dir = project("dbl");
    let root = dir.path().join("dbl");
    fs::write(root.join(".bashrc"), "a\n").unwrap();
    let shell = mount(cx, &root, &dir.path().join("gui.json"));
    press(&shell, cx, "t");
    press(&shell, cx, "n");
    press(&shell, cx, "enter");

    let export_path = dir.path().join("out.json");
    let dummy = dir.path().join("nope.json");
    fs::write(&dummy, "{}\n").unwrap();
    shell.update(cx, |shell, _| {
        let app = shell.app.as_mut().unwrap();
        app.state.modal = Some(Modal::ExportPicker {
            dir: dir.path().to_path_buf(),
            filename: "out".to_string(),
        });
    });
    press(&shell, cx, "enter");

    shell.update(cx, |shell, _| {
        let app = shell.app.as_mut().unwrap();
        if let Some(node) =
            dd_dotstore::tree::find_mut_node(&mut app.state.tree, Path::new(".bashrc"))
        {
            node.group = None;
        }
        dd_dotstore::tree::flatten_visible(&mut app.state);
        app.state.modal = Some(Modal::ImportPicker {
            files: vec![dummy.clone(), export_path.clone()],
            selected: 0,
        });
    });
    shell.update(cx, |shell, _| shell.click_modal_row(1, 1));
    let body = text(&shell, cx);
    assert!(body.contains("modal:import"), "{body}");
    assert!(body.contains("> out.json"), "{body}");
    assert!(!body.contains("·n"), "{body}");
    shell.update(cx, |shell, _| shell.click_modal_row(1, 2));
    let body = text(&shell, cx);
    assert!(!body.contains("modal:import"), "{body}");
    assert!(body.contains("·n"), "{body}");
}
