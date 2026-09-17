mod common;

use common::{TempRoot, key};
use crossterm::event::KeyCode;
use dd_dotstore::app::App;
use dd_dotstore::domain::Modal;
use dd_dotstore::inputs::handle_key;
use dd_dotstore::theme::{
    COLOR_FIELDS, Theme, ThemeColors, ThemeSaveTarget, ThemeSource, color_from_rgb, color_to_hex,
    load_theme_with, nudge_channel, parse_hex_input, render_theme_yaml, save_theme,
};
use ratatui::style::Color;
use std::fs;

fn all_required_colors() -> String {
    let mut body = String::from("version: 1\ncolors:\n");
    for field in COLOR_FIELDS {
        body.push_str(&format!("  {}: \"#010203\"\n", field.key));
    }
    body
}

#[test]
fn color_field_list_covers_every_required_key() {
    assert_eq!(COLOR_FIELDS.len(), 26);
    let yaml = all_required_colors();
    let root = TempRoot::create("theme_keys");
    fs::write(root.join("dd_dotstore_theme.yml"), yaml).expect("write");
    let theme = load_theme_with(&root, None).expect("load");
    assert_eq!(theme.source, ThemeSource::Local);
}

#[test]
fn parse_hex_input_accepts_hash_and_bare() {
    assert_eq!(
        color_from_rgb(parse_hex_input("#AABBCC").expect("hash")),
        Color::Rgb(0xaa, 0xbb, 0xcc)
    );
    assert_eq!(
        color_from_rgb(parse_hex_input("aabbcc").expect("bare")),
        Color::Rgb(0xaa, 0xbb, 0xcc)
    );
    assert!(parse_hex_input("gg0000").is_err());
    assert!(parse_hex_input("#fff").is_err());
}

#[test]
fn nudge_channel_clamps() {
    let color = Color::Rgb(250, 0, 10);
    assert_eq!(nudge_channel(color, 0, 8), Color::Rgb(255, 0, 10));
    assert_eq!(nudge_channel(color, 1, -8), Color::Rgb(250, 0, 10));
    assert_eq!(nudge_channel(color, 2, 5), Color::Rgb(250, 0, 15));
}

#[test]
fn c_opens_editor_with_global_save_default() {
    let root = TempRoot::create("theme_open");
    fs::write(root.join(".bashrc"), "x").expect("write");
    let mut app = App::new_with_root(&root).expect("app");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('C'))).expect("C");
    match &app.state.modal {
        Some(Modal::ThemeEditor(editor)) => {
            assert_eq!(editor.save_target, ThemeSaveTarget::Global);
            assert!(!editor.editing_hex);
        }
        other => panic!("expected ThemeEditor, got {other:?}"),
    }
}

#[test]
fn tab_toggles_save_target_and_esc_reverts_colors() {
    let root = TempRoot::create("theme_esc");
    fs::write(root.join(".bashrc"), "x").expect("write");
    let mut app = App::new_with_root(&root).expect("app");
    let original = app.state.theme.colors;
    let _ = handle_key(&mut app.state, key(KeyCode::Char('C'))).expect("C");
    let _ = handle_key(&mut app.state, key(KeyCode::Tab)).expect("tab");
    match &app.state.modal {
        Some(Modal::ThemeEditor(editor)) => {
            assert_eq!(editor.save_target, ThemeSaveTarget::Local)
        }
        other => panic!("expected ThemeEditor, got {other:?}"),
    }
    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("nudge");
    assert_ne!(
        app.state.theme.colors.base_background,
        original.base_background
    );
    let _ = handle_key(&mut app.state, key(KeyCode::Esc)).expect("esc");
    assert!(app.state.modal.is_none());
    assert_eq!(app.state.theme.colors, original);
}

#[test]
fn save_local_and_global_round_trip() {
    let project = TempRoot::create("theme_save_proj");
    let xdg = TempRoot::create("theme_save_xdg");
    let colors = ThemeColors {
        folders: Color::Rgb(0x11, 0x22, 0x33),
        ..ThemeColors::default()
    };
    let theme = Theme {
        header_quotes: vec!["Keep me.".into()],
        ..Theme::from_colors(colors, ThemeSource::Default, 1)
    };

    let local = save_theme(
        &theme,
        &project,
        ThemeSaveTarget::Local,
        Some(xdg.as_path()),
    )
    .expect("save local");
    assert!(local.ends_with("dd_dotstore_theme.yml"));
    let loaded_local = load_theme_with(&project, Some(xdg.as_path())).expect("load local");
    assert_eq!(loaded_local.source, ThemeSource::Local);
    assert_eq!(loaded_local.colors.folders, Color::Rgb(0x11, 0x22, 0x33));
    assert_eq!(loaded_local.header_quotes, vec!["Keep me.".to_string()]);

    fs::remove_file(&local).expect("remove local so global can win");
    let global = save_theme(
        &theme,
        &project,
        ThemeSaveTarget::Global,
        Some(xdg.as_path()),
    )
    .expect("save global");
    assert!(global.display().to_string().contains("ldnddev"));
    let loaded_global = load_theme_with(&project, Some(xdg.as_path())).expect("load global");
    assert_eq!(loaded_global.source, ThemeSource::Global);
    assert_eq!(loaded_global.colors.folders, Color::Rgb(0x11, 0x22, 0x33));
}

#[test]
fn render_yaml_keeps_quotes_and_hex() {
    let theme = Theme {
        header_quotes: vec!["Don't Fear the . (Dot) - Tame It.".into()],
        ..Theme::default()
    };
    let yaml = render_theme_yaml(&theme);
    assert!(yaml.starts_with("version: 1\n"));
    assert!(yaml.contains("header_quotes:"));
    assert!(yaml.contains(&color_to_hex(ThemeColors::default().folders)));
    assert!(yaml.contains("base_background:"));
}

#[test]
fn reset_restores_builtin_colors_without_closing() {
    let root = TempRoot::create("theme_reset");
    fs::write(root.join(".bashrc"), "x").expect("write");
    let mut app = App::new_with_root(&root).expect("app");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('C'))).expect("C");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("nudge");
    assert_ne!(
        app.state.theme.colors.base_background,
        ThemeColors::default().base_background
    );
    let _ = handle_key(&mut app.state, key(KeyCode::Char('R'))).expect("reset");
    assert!(matches!(app.state.modal, Some(Modal::ThemeEditor(_))));
    assert_eq!(app.state.theme.colors, ThemeColors::default());
}
