mod common;

use common::{TempRoot, key, mouse_left};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_dotstore::actions::{
    action_targets, assign_destination, collect_preview_lines, create_copy, create_symlink,
    default_dest_with, push_history, remove_symlink, selected_create_conflicts, undo_last,
};
use dd_dotstore::app::App;
use dd_dotstore::inputs::{handle_key, handle_mouse};
use dd_dotstore::state::{
    Action, BrowserState, BulkAction, DirEntry, Modal, NodeKind, SymlinkStatus, ThemeSource,
    load_theme,
};
use dd_dotstore::toast::{TOAST_DURATION, ToastLevel};
use dd_dotstore::tree::{build_tree, find_node, flatten_visible};
use dd_dotstore::ui::{overwrite_conflict_row, overwrite_warning_counts};
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[test]
fn toast_expires_after_five_seconds() {
    let root = TempRoot::create("toast_expiry");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.show_toast(ToastLevel::Info, "Saved");

    let toast = app.state.toast.as_mut().expect("toast exists");
    assert_eq!(toast.duration, TOAST_DURATION);
    toast.created_at = toast
        .created_at
        .checked_sub(TOAST_DURATION + Duration::from_millis(1))
        .expect("rewind toast");

    app.tick();
    assert!(app.state.toast.is_none());
}

#[test]
fn build_tree_respects_ignore_patterns() {
    let root = TempRoot::create("ignore");
    fs::create_dir_all(root.join(".git")).expect("create .git");
    fs::write(root.join(".git/config"), "x").expect("write git config");
    fs::write(root.join(".bashrc"), "alias ll='ls -la'").expect("write dotfile");

    let tree = build_tree(&root, &[".git".to_string()]);

    assert!(find_node(&tree, Path::new(".git")).is_none());
    assert!(find_node(&tree, Path::new(".bashrc")).is_some());
}

#[test]
fn app_initializes_with_tree_nodes() {
    let root = TempRoot::create("init");
    fs::write(root.join(".zshrc"), "export PATH=$PATH").expect("write dotfile");

    let app = App::new_with_root(&root).expect("app init");
    assert!(!app.state.tree.is_empty());
    assert!(!app.state.nodes.is_empty());
}

#[test]
fn local_standard_theme_file_is_loaded() {
    let root = TempRoot::create("theme_local");
    fs::write(
        root.join("dd_dotstore_theme.yml"),
        r##"
version: 1
header_quotes:
  - "Custom quote 1"
  - "Custom quote 2"
colors:
  base_background: "#010203"
  body_background: "#111213"
  modal_background: "#212223"
  text_primary: "#313233"
  text_secondary: "#414243"
  text_labels: "#515253"
  text_active_focus: "#616263"
  modal_labels: "#717273"
  modal_text: "#818283"
  selected_background: "#919293"
  border_default: "#A1A2A3"
  border_active: "#B1B2B3"
  scrollbar: "#C1C2C3"
  scrollbar_hover: "#D1D2D3"
  input_border_default: "#E1E2E3"
  input_border_focus: "#F1F2F3"
  input_text_default: "#0A0B0C"
  input_text_focus: "#1A1B1C"
  cursor: "#2A2B2C"
  success: "#3A3B3C"
  warning: "#4A4B4C"
  error: "#5A5B5C"
  info: "#6A6B6C"
  folders: "#7A7B7C"
  files: "#8A8B8C"
  links: "#9A9B9C"
"##,
    )
    .expect("write theme");

    let theme = load_theme(&root).expect("load theme");
    assert_eq!(theme.source, ThemeSource::Local);
    assert_eq!(theme.version, 1);
    assert_eq!(theme.colors.base_background, Color::Rgb(1, 2, 3));
    assert_eq!(theme.colors.border_active, Color::Rgb(0xb1, 0xb2, 0xb3));
    assert_eq!(theme.colors.links, Color::Rgb(0x9a, 0x9b, 0x9c));
    assert_eq!(
        theme.header_quotes,
        vec!["Custom quote 1".to_string(), "Custom quote 2".to_string()]
    );
}

#[test]
fn theme_requires_schema_version() {
    let root = TempRoot::create("theme_missing_version");
    fs::write(
        root.join("dd_dotstore_theme.yml"),
        r##"
colors:
  base_background: "#010203"
"##,
    )
    .expect("write theme");

    let err = match load_theme(&root) {
        Ok(_) => panic!("missing version should fail"),
        Err(err) => err,
    };
    let full_error = format!("{err:#}");
    assert!(full_error.contains("Missing required theme key `version`"));
}

#[test]
fn app_falls_back_to_default_theme_when_theme_version_is_unsupported() {
    let root = TempRoot::create("theme_unsupported_version");
    fs::write(
        root.join("dd_dotstore_theme.yml"),
        r##"
version: 999
colors:
  base_background: "#010203"
"##,
    )
    .expect("write theme");

    let app = App::new_with_root(&root).expect("app init falls back");
    assert_eq!(app.state.theme.source, ThemeSource::Default);
    assert!(
        app.state
            .theme_status
            .message
            .contains("Unsupported theme schema version")
    );
    assert_eq!(app.state.theme.header_quotes.len(), 5);
    assert!(
        app.state
            .theme
            .header_quotes
            .iter()
            .any(|q| q.contains("Ricer"))
    );
}

#[test]
fn create_and_remove_symlink_round_trip() {
    let root = TempRoot::create("links");
    fs::write(root.join(".vimrc"), "set number").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");

    let rel = Path::new(".vimrc");
    let dest = root.join("home/.vimrc");

    let created = create_symlink(&mut app.state, rel, &dest).expect("create symlink");
    assert!(created);
    assert!(
        fs::symlink_metadata(&dest)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    );

    let node = find_node(&app.state.tree, rel).expect("find file node");
    match &node.kind {
        NodeKind::File { dest: node_dest } => {
            assert_eq!(node_dest.as_ref(), Some(&dest));
        }
        NodeKind::Folder { .. } => panic!("expected file node"),
    }

    let removed = remove_symlink(&mut app.state, rel).expect("remove symlink");
    assert!(removed);
    assert!(!dest.exists());
}

#[test]
fn undo_history_is_capped_and_undo_reverts_creation() {
    let root = TempRoot::create("undo");
    fs::write(root.join(".gitconfig"), "[user]").expect("write dotfile");
    let mut app = App::new_with_root(&root).expect("app init");

    for i in 0..12 {
        push_history(
            &mut app.state,
            Action::Create {
                src: PathBuf::from(format!("src_{i}")),
                dest: PathBuf::from(format!("/tmp/dest_{i}")),
            },
        );
    }
    assert_eq!(app.state.history.len(), 10);
    match app.state.history.front().expect("front action") {
        Action::Create { src, .. } => assert_eq!(src, &PathBuf::from("src_2")),
        Action::Remove { .. } => panic!("unexpected remove action"),
        Action::Copy { .. } | Action::RemoveCopy { .. } => panic!("unexpected copy action"),
    }

    let rel = Path::new(".gitconfig");
    let dest = root.join("home/.gitconfig");
    create_symlink(&mut app.state, rel, &dest).expect("create symlink");
    undo_last(&mut app.state).expect("undo create");
    assert!(!dest.exists());

    let node = find_node(&app.state.tree, rel).expect("find node after undo");
    match &node.kind {
        NodeKind::File { dest: node_dest } => assert!(node_dest.is_none()),
        NodeKind::Folder { .. } => panic!("expected file node"),
    }
}

#[test]
fn create_copy_copies_file_and_undo_removes_copy() {
    let root = TempRoot::create("copy_file");
    fs::write(root.join(".editorconfig"), "root = true").expect("write source");

    let mut app = App::new_with_root(&root).expect("app init");
    let rel = Path::new(".editorconfig");
    let dest = root.join("home/.editorconfig");

    let copied = create_copy(&mut app.state, rel, &dest).expect("copy file");
    assert!(copied);
    assert_eq!(fs::read_to_string(&dest).expect("read copy"), "root = true");
    assert!(
        fs::symlink_metadata(&dest)
            .map(|m| !m.file_type().is_symlink())
            .unwrap_or(false)
    );

    undo_last(&mut app.state).expect("undo copy");
    assert!(!dest.exists());
}

#[test]
fn bulk_copy_supports_selected_folders() {
    let root = TempRoot::create("copy_folder");
    fs::create_dir_all(root.join("nvim/lua")).expect("create source dirs");
    fs::write(root.join("nvim/init.lua"), "vim.opt.number = true").expect("write init");
    fs::write(root.join("nvim/lua/plugins.lua"), "return {}").expect("write plugin");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("nvim"))
        .expect("find folder index");

    let dest = root.join("copied_nvim");
    assign_destination(&mut app.state, Path::new("nvim"), dest.clone()).expect("assign dest");

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('m'))).expect("toggle mode");
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("select folder");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("open bulk create");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm create");

    let copied = dest.join("lua/plugins.lua");
    assert_eq!(
        fs::read_to_string(copied).expect("read nested copy"),
        "return {}"
    );
}

#[test]
fn modal_edit_dest_enter_selects_destination_file() {
    let root = TempRoot::create("modal_dest");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write dotfile");
    fs::write(root.join("target_file"), "placeholder").expect("write target");

    let mut app = App::new_with_root(&root).expect("app init");
    let node_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("find .bashrc index");

    let browser = BrowserState {
        root: root.clone(),
        current: root.clone(),
        entries: vec![DirEntry {
            name: "target_file".to_string(),
            is_dir: false,
        }],
        selected: 0,
        filter: String::new(),
    };
    app.state.modal = Some(Modal::EditDest { node_idx, browser });

    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("handle key");

    assert!(app.state.modal.is_none());
    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert_eq!(dest.as_ref(), Some(&root.join("target_file"))),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
}

#[test]
fn modal_import_and_export_picker_key_paths_work() {
    let root = TempRoot::create("picker");
    fs::write(root.join(".profile"), "export PATH=$PATH").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");

    let import_a = root.join("import_a.json");
    let import_b = root.join("import_b.json");
    fs::write(
        &import_a,
        r#"{"symlinks":{".profile":"/tmp/dest_a"},"history":[]}"#,
    )
    .expect("write import_a");
    fs::write(
        &import_b,
        r#"{"symlinks":{".profile":"/tmp/dest_b"},"history":[]}"#,
    )
    .expect("write import_b");

    app.state.modal = Some(Modal::ImportPicker {
        files: vec![import_a, import_b],
        selected: 0,
    });
    let _ = handle_key(&mut app.state, key(KeyCode::Down)).expect("down key");
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("enter key");

    let node = find_node(&app.state.tree, Path::new(".profile")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert_eq!(dest.as_ref(), Some(&PathBuf::from("/tmp/dest_b"))),
        NodeKind::Folder { .. } => panic!("expected file"),
    }

    app.state.modal = Some(Modal::ExportPicker {
        dir: root.clone(),
        filename: "modal_export".to_string(),
    });
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("export enter");
    assert!(root.join("modal_export.json").exists());
}

#[test]
fn malformed_import_shows_error_toast_instead_of_failing_key_handler() {
    let root = TempRoot::create("bad_import");
    fs::write(root.join(".profile"), "export PATH=$PATH").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");
    let bad = root.join("bad_import.json");
    fs::write(&bad, "{this-is: not-json").expect("write malformed json");

    app.state.modal = Some(Modal::ImportPicker {
        files: vec![bad],
        selected: 0,
    });
    let handled = handle_key(&mut app.state, key(KeyCode::Enter));
    assert!(
        handled.is_ok(),
        "import parse failure should not bubble as key error"
    );
    assert!(app.state.modal.is_none());
    assert!(matches!(
        app.state.toast.as_ref().map(|toast| toast.level),
        Some(ToastLevel::Error)
    ));
}

#[test]
fn esc_closes_common_modals() {
    let root = TempRoot::create("modal_esc");
    fs::write(root.join(".vimrc"), "set number").expect("write dotfile");
    let mut app = App::new_with_root(&root).expect("app init");
    let node_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".vimrc"))
        .expect("find .vimrc");

    let modals = vec![
        Modal::Help,
        Modal::Credits,
        Modal::IgnoreEditor {
            selected: 0,
            draft: String::new(),
        },
        Modal::ImportPicker {
            files: vec![root.join("x.json")],
            selected: 0,
        },
        Modal::ExportPicker {
            dir: root.clone(),
            filename: "x".to_string(),
        },
        Modal::EditDest {
            node_idx,
            browser: BrowserState {
                root: root.clone(),
                current: root.clone(),
                entries: vec![],
                selected: 0,
                filter: String::new(),
            },
        },
    ];

    for modal in modals {
        app.state.modal = Some(modal);
        let _ = handle_key(&mut app.state, key(KeyCode::Esc)).expect("esc handler");
        assert!(app.state.modal.is_none(), "Esc should close modal");
    }
}

#[test]
fn f2_opens_and_closes_credits_modal() {
    let root = TempRoot::create("credits");
    fs::write(root.join(".xinitrc"), "exec awesome").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");
    let _ = handle_key(&mut app.state, key(KeyCode::F(2))).expect("open credits");
    assert!(matches!(app.state.modal, Some(Modal::Credits)));
    let _ = handle_key(&mut app.state, key(KeyCode::F(2))).expect("close credits");
    assert!(app.state.modal.is_none());
}

#[test]
fn modal_edit_dest_fuzzy_filter_selects_matching_entry() {
    let root = TempRoot::create("modal_fuzzy");
    fs::write(root.join(".inputrc"), "set completion-ignore-case on").expect("write dotfile");
    fs::write(root.join("alpha_target"), "a").expect("write alpha");
    fs::write(root.join("beta_target"), "b").expect("write beta");

    let mut app = App::new_with_root(&root).expect("app init");
    let node_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".inputrc"))
        .expect("find .inputrc index");

    let browser = BrowserState {
        root: root.clone(),
        current: root.clone(),
        entries: vec![
            DirEntry {
                name: "alpha_target".to_string(),
                is_dir: false,
            },
            DirEntry {
                name: "beta_target".to_string(),
                is_dir: false,
            },
        ],
        selected: 0,
        filter: String::new(),
    };
    app.state.modal = Some(Modal::EditDest { node_idx, browser });

    let _ = handle_key(&mut app.state, key(KeyCode::Char('b'))).expect("type b");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('t'))).expect("type t");
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("choose filtered");

    let node = find_node(&app.state.tree, Path::new(".inputrc")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert_eq!(dest.as_ref(), Some(&root.join("beta_target"))),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
}

#[test]
fn modal_edit_dest_supports_home_and_jump_shortcuts() {
    let root = TempRoot::create("modal_shortcuts");
    fs::create_dir_all(root.join("nested")).expect("create nested");
    fs::write(root.join(".zshenv"), "export FOO=bar").expect("write dotfile");
    fs::write(root.join("nested/a"), "a").expect("write a");
    fs::write(root.join("nested/b"), "b").expect("write b");

    let mut app = App::new_with_root(&root).expect("app init");
    let node_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".zshenv"))
        .expect("find .zshenv");

    let mut browser = BrowserState {
        root: root.clone(),
        current: root.join("nested"),
        entries: vec![],
        selected: 0,
        filter: String::new(),
    };
    browser.refresh_entries();
    app.state.modal = Some(Modal::EditDest { node_idx, browser });

    let _ = handle_key(&mut app.state, key(KeyCode::Char('~'))).expect("home shortcut");
    if let Some(Modal::EditDest { browser, .. }) = &app.state.modal {
        assert_eq!(browser.current, root.path);
    } else {
        panic!("expected EditDest modal");
    }

    let _ = handle_key(&mut app.state, key(KeyCode::Char('G'))).expect("jump end");
    if let Some(Modal::EditDest { browser, .. }) = &app.state.modal {
        let filtered = browser.filtered_indices();
        assert_eq!(browser.selected, filtered.len().saturating_sub(1));
    } else {
        panic!("expected EditDest modal");
    }
}

#[test]
fn cannot_change_destination_while_existing_symlink_is_present() {
    let root = TempRoot::create("dest_lock");
    fs::write(root.join(".zprofile"), "export ZDOTDIR=$HOME").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");
    let rel = Path::new(".zprofile");
    let old_dest = root.join("home/.zprofile");
    let new_dest = root.join("home/.zprofile_new");

    create_symlink(&mut app.state, rel, &old_dest).expect("create initial symlink");
    assign_destination(&mut app.state, rel, new_dest.clone()).expect("attempt change destination");

    let node = find_node(&app.state.tree, rel).expect("find node");
    match &node.kind {
        NodeKind::File { dest } => assert_eq!(dest.as_ref(), Some(&old_dest)),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
    assert!(matches!(
        app.state.toast.as_ref().map(|toast| toast.level),
        Some(ToastLevel::Error)
    ));
    assert!(old_dest.exists());
    assert!(!new_dest.exists());
}

#[test]
fn bulk_create_conflict_requires_overwrite_confirmation() {
    let root = TempRoot::create("overwrite_warning");
    fs::write(root.join(".bash_profile"), "export HISTSIZE=10000").expect("write source");
    fs::create_dir_all(root.join("home")).expect("create home dir");

    let mut app = App::new_with_root(&root).expect("app init");
    let rel = Path::new(".bash_profile");
    let dest = root.join("home/.bash_profile");
    fs::write(&dest, "existing real file").expect("write existing destination file");

    assign_destination(&mut app.state, rel, dest.clone()).expect("set destination");

    if let Some(node) = app
        .state
        .nodes
        .iter_mut()
        .find(|n| matches!(n.kind, NodeKind::File { .. }) && n.path == rel)
    {
        node.selected = true;
    }

    app.state.modal = Some(Modal::Plan {
        action: dd_dotstore::state::BulkAction::Create,
        scroll: 0,
    });
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm create");
    assert!(matches!(
        app.state.modal,
        Some(Modal::OverwriteWarning { .. })
    ));
    assert!(
        fs::symlink_metadata(&dest)
            .map(|m| !m.file_type().is_symlink())
            .unwrap_or(false)
    );

    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm overwrite");
    assert!(app.state.modal.is_none());
    assert!(
        fs::symlink_metadata(&dest)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    );
}

#[test]
fn space_toggles_folder_selection_in_main_view() {
    let root = TempRoot::create("folder_select");
    fs::create_dir_all(root.join("nvim")).expect("create nvim dir");
    fs::write(root.join("nvim/init.lua"), "return {}").expect("write file");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("nvim"))
        .expect("find folder index");

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("space");

    let node = find_node(&app.state.tree, Path::new("nvim")).expect("find folder node");
    assert!(node.selected, "space should toggle folder selection");
}

#[test]
fn hl_toggles_folder_expand_collapse() {
    let root = TempRoot::create("folder_expand");
    fs::create_dir_all(root.join("tmux")).expect("create tmux dir");
    fs::write(root.join("tmux/tmux.conf"), "set -g mouse on").expect("write file");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("tmux"))
        .expect("find folder index");

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("l expand");
    let expanded_node = find_node(&app.state.tree, Path::new("tmux")).expect("expanded node");
    match &expanded_node.kind {
        NodeKind::Folder { expanded, .. } => assert!(*expanded),
        NodeKind::File { .. } => panic!("expected folder"),
    }

    let _ = handle_key(&mut app.state, key(KeyCode::Char('h'))).expect("h collapse");
    let collapsed_node = find_node(&app.state.tree, Path::new("tmux")).expect("collapsed node");
    match &collapsed_node.kind {
        NodeKind::Folder { expanded, .. } => assert!(!*expanded),
        NodeKind::File { .. } => panic!("expected folder"),
    }
}

#[test]
fn enter_on_folder_opens_destination_editor() {
    let root = TempRoot::create("folder_enter_dest");
    fs::create_dir_all(root.join("kitty")).expect("create kitty dir");
    fs::create_dir_all(root.join("home")).expect("create home dir");
    fs::write(root.join("kitty/kitty.conf"), "font_family FiraCode").expect("write file");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("kitty"))
        .expect("find folder index");

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("enter on folder");
    assert!(matches!(app.state.modal, Some(Modal::EditDest { .. })));
    if let Some(Modal::EditDest { browser, .. }) = &mut app.state.modal {
        browser.current = root.join("home");
        browser.refresh_entries();
    } else {
        panic!("expected destination modal");
    }
    let _ = handle_key(
        &mut app.state,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    )
    .expect("ctrl+s choose current dir");

    let node = find_node(&app.state.tree, Path::new("kitty")).expect("folder node");
    match &node.kind {
        NodeKind::Folder { dest, .. } => assert_eq!(dest.as_ref(), Some(&root.join("home/kitty"))),
        NodeKind::File { .. } => panic!("expected folder"),
    }
}

#[test]
fn bulk_create_and_remove_supports_selected_folders() {
    let root = TempRoot::create("folder_bulk");
    fs::create_dir_all(root.join("alacritty")).expect("create alacritty dir");
    fs::write(root.join("alacritty/alacritty.toml"), "[window]").expect("write file");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("alacritty"))
        .expect("find folder index");
    let folder_link = root.join("linked_alacritty");
    assign_destination(&mut app.state, Path::new("alacritty"), folder_link.clone())
        .expect("assign dest");

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("select folder");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("open bulk create");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm create");

    assert!(
        fs::symlink_metadata(&folder_link)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "expected folder symlink at assigned destination"
    );

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("reselect folder");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('x'))).expect("open bulk remove");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm remove");

    assert!(
        fs::symlink_metadata(&folder_link).is_err(),
        "expected folder symlink to be removed"
    );
}

#[test]
fn shift_click_range_selects_from_previous_highlight() {
    let root = TempRoot::create("shift_click");
    fs::write(root.join("alpha"), "a").expect("write alpha");
    fs::write(root.join("bravo"), "b").expect("write bravo");
    fs::write(root.join("charlie"), "c").expect("write charlie");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.pointer.source_area = Rect::new(0, 0, 40, 20);
    let (i0, i1, i2) = {
        let idx = |name: &str| {
            app.state
                .nodes
                .iter()
                .position(|n| n.path == Path::new(name))
                .expect(name)
        };
        (idx("alpha"), idx("bravo"), idx("charlie"))
    };
    app.state.list_state.select(Some(i0));

    // inner_y = 1, so row = 1 + index
    let _ = handle_mouse(&mut app.state, mouse_left(20, 1 + i2 as u16, true))
        .expect("shift-click range");

    assert!(
        find_node(&app.state.tree, Path::new("alpha"))
            .unwrap()
            .selected
    );
    assert!(
        find_node(&app.state.tree, Path::new("bravo"))
            .unwrap()
            .selected,
        "middle row must be included (bug was cur == idx after moving highlight)"
    );
    assert!(
        find_node(&app.state.tree, Path::new("charlie"))
            .unwrap()
            .selected
    );
    assert_eq!(app.state.list_state.selected(), Some(i2));
    assert_eq!(i1, i0 + 1);
}

#[test]
fn esc_does_not_quit_main_view() {
    let root = TempRoot::create("esc_no_quit");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.filter = "nope".to_string();
    flatten_visible(&mut app.state);

    let quit = handle_key(&mut app.state, key(KeyCode::Esc)).expect("esc filter");
    assert!(!quit, "Esc must not quit");
    assert!(app.state.filter.is_empty());
    assert!(app.state.modal.is_none());

    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("find bashrc");
    app.state.list_state.select(Some(idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("select");
    assert!(
        find_node(&app.state.tree, Path::new(".bashrc"))
            .unwrap()
            .selected
    );

    let quit = handle_key(&mut app.state, key(KeyCode::Esc)).expect("esc selection");
    assert!(!quit, "Esc must not quit");
    assert!(
        !find_node(&app.state.tree, Path::new(".bashrc"))
            .unwrap()
            .selected
    );

    let quit = handle_key(&mut app.state, key(KeyCode::Esc)).expect("esc noop");
    assert!(!quit, "Esc must not quit when nothing to clear");
}

#[test]
#[allow(non_snake_case)]
fn q_and_Q_quit() {
    let root = TempRoot::create("q_quit");
    fs::write(root.join(".zshrc"), "export PATH=$PATH").expect("write zshrc");

    let mut app = App::new_with_root(&root).expect("app init");
    assert!(handle_key(&mut app.state, key(KeyCode::Char('q'))).expect("q"));
    assert!(handle_key(&mut app.state, key(KeyCode::Char('Q'))).expect("Q"));
}

#[test]
fn overwrite_lists_all_dirs_then_files() {
    let root = TempRoot::create("overwrite_all_dirs");
    let dest_root = root.join("home");
    fs::create_dir_all(&dest_root).expect("create dest root");

    let dir_names = ["dir_a", "dir_b", "dir_c", "dir_d", "dir_e"];
    for name in dir_names {
        fs::create_dir_all(root.join(name)).expect("src dir");
        fs::create_dir_all(dest_root.join(name)).expect("real dest dir");
    }
    for name in ["file_f", "file_g"] {
        fs::write(root.join(name), "src").expect("src file");
        fs::write(dest_root.join(name), "existing").expect("real dest file");
    }

    let mut app = App::new_with_root(&root).expect("app init");
    for name in dir_names.iter().chain(["file_f", "file_g"].iter()) {
        assign_destination(&mut app.state, Path::new(name), dest_root.join(name))
            .expect("assign dest");
    }
    for node in app.state.nodes.iter_mut() {
        node.selected = true;
    }

    let conflicts = selected_create_conflicts(&app.state);
    let (dirs, files, total) = overwrite_warning_counts(&conflicts);
    assert_eq!((dirs, files, total), (5, 2, 7));
    assert!(conflicts.iter().take(5).all(|c| c.is_dir));
    assert!(conflicts.iter().skip(5).all(|c| !c.is_dir));
    for name in dir_names {
        assert!(
            conflicts
                .iter()
                .any(|c| c.dest == dest_root.join(name) && c.is_dir),
            "missing DIR {name}"
        );
    }
    let rows: Vec<String> = conflicts.iter().map(overwrite_conflict_row).collect();
    assert_eq!(rows.iter().filter(|r| r.contains("DIR")).count(), 5);
    assert_eq!(rows.iter().filter(|r| r.contains("FILE")).count(), 2);

    app.state.modal = Some(Modal::Plan {
        action: BulkAction::Create,
        scroll: 0,
    });
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("open overwrite");
    match &app.state.modal {
        Some(Modal::OverwriteWarning {
            conflicts: modal_conflicts,
            scroll,
            ..
        }) => {
            assert_eq!(modal_conflicts.len(), 7);
            assert_eq!(modal_conflicts.iter().filter(|c| c.is_dir).count(), 5);
            assert_eq!(*scroll, 0);
        }
        other => panic!("expected OverwriteWarning, got {other:?}"),
    }

    let _ = handle_key(&mut app.state, key(KeyCode::Char('j'))).expect("scroll down");
    match &app.state.modal {
        Some(Modal::OverwriteWarning { scroll, .. }) => assert_eq!(*scroll, 1),
        other => panic!("j must not cancel overwrite, got {other:?}"),
    }
    let _ = handle_key(&mut app.state, key(KeyCode::Char('k'))).expect("scroll up");
    match &app.state.modal {
        Some(Modal::OverwriteWarning { scroll, .. }) => assert_eq!(*scroll, 0),
        other => panic!("k must not cancel overwrite, got {other:?}"),
    }
}

#[test]
fn click_inside_confirm_overwrite_preview_is_noop() {
    let root = TempRoot::create("click_inside_noop");
    fs::write(root.join(".profile"), "export PATH=$PATH").expect("write profile");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.pointer.current_modal_area = Some(Rect::new(10, 10, 40, 20));

    for modal in [
        Modal::Plan {
            action: BulkAction::Create,
            scroll: 0,
        },
        Modal::OverwriteWarning {
            conflicts: vec![],
            action_type: BulkAction::Create,
            scroll: 0,
        },
    ] {
        app.state.modal = Some(modal);
        let _ = handle_mouse(&mut app.state, mouse_left(20, 15, false)).expect("inside click");
        assert!(
            app.state.modal.is_some(),
            "click inside confirm/overwrite/plan must be a no-op"
        );
    }

    app.state.modal = Some(Modal::Plan {
        action: BulkAction::Create,
        scroll: 0,
    });
    let _ = handle_mouse(&mut app.state, mouse_left(0, 0, false)).expect("outside click");
    assert!(app.state.modal.is_none(), "click outside must still cancel");
}

#[test]
fn default_dest_with_mapping_table() {
    let home = Path::new("/tmp/fakehome");
    let xdg = Path::new("/tmp/fakexdg");
    let project = Path::new("/tmp/proj");
    let map =
        |rel: &str, is_dir: bool| default_dest_with(home, xdg, project, Path::new(rel), is_dir);

    assert_eq!(
        map(".bashrc", false),
        PathBuf::from("/tmp/fakehome/.bashrc")
    );
    assert_eq!(map("nvim", true), PathBuf::from("/tmp/fakexdg/nvim"));
    assert_eq!(map(".config/foo", false), PathBuf::from("/tmp/fakexdg/foo"));
    assert_eq!(map(".config/foo", true), PathBuf::from("/tmp/fakexdg/foo"));
    assert_eq!(
        map(".config/nvim/init.lua", false),
        PathBuf::from("/tmp/fakexdg/nvim/init.lua")
    );
    assert_eq!(map(".ssh", true), PathBuf::from("/tmp/fakehome/.ssh"));
    assert_eq!(map(".config", true), PathBuf::from("/tmp/fakexdg"));
    assert_eq!(
        map(".config", false),
        PathBuf::from("/tmp/fakehome/.config")
    );
    assert_eq!(
        map("README.md", false),
        PathBuf::from("/tmp/fakehome/README.md")
    );
    assert_eq!(
        map("scripts/setup.sh", false),
        PathBuf::from("/tmp/proj/.linked/scripts/setup.sh")
    );
}

#[test]
fn default_ignores_skip_linked_dir() {
    let root = TempRoot::create("ignore_linked");
    fs::create_dir_all(root.join(".linked")).expect("create .linked");
    fs::write(root.join(".linked/foo"), "x").expect("write linked file");
    fs::write(root.join(".bashrc"), "alias ll='ls -la'").expect("write bashrc");

    let app = App::new_with_root(&root).expect("app init");
    assert!(app.state.ignore_patterns.iter().any(|p| p == ".linked"));
    assert!(find_node(&app.state.tree, Path::new(".linked")).is_none());
    assert!(find_node(&app.state.tree, Path::new(".bashrc")).is_some());
}

#[test]
fn assigned_missing_dest_is_planned_status() {
    let root = TempRoot::create("planned_status");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let dest = root.join("missing_home/.bashrc");
    assign_destination(&mut app.state, Path::new(".bashrc"), dest).expect("assign dest");

    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("find node");
    assert_eq!(node.symlink_status, SymlinkStatus::Planned);
}

#[test]
fn metadata_err_other_than_notfound_is_unknown() {
    let root = TempRoot::create("unknown_status");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");
    let blocker = root.join("not_a_dir");
    fs::write(&blocker, "x").expect("write file-as-parent");
    let dest = blocker.join("child");

    let mut app = App::new_with_root(&root).expect("app init");
    assign_destination(&mut app.state, Path::new(".bashrc"), dest).expect("assign dest");

    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("find node");
    assert_eq!(node.symlink_status, SymlinkStatus::Unknown);
}

#[test]
fn s_and_p_open_same_plan_on_highlight_without_checkbox() {
    let root = TempRoot::create("highlight_plan");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("find bashrc");
    app.state.list_state.select(Some(idx));
    assert!(!app.state.nodes[idx].selected);

    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("s");
    assert!(
        matches!(
            app.state.modal,
            Some(Modal::Plan {
                action: BulkAction::Create,
                scroll: 0
            })
        ),
        "s must open Plan apply"
    );
    assert!(
        !app.state.nodes[idx].selected,
        "s must not flip the checkbox"
    );

    let _ = handle_key(&mut app.state, key(KeyCode::Esc)).expect("close plan");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('p'))).expect("p");
    assert!(matches!(
        app.state.modal,
        Some(Modal::Plan {
            action: BulkAction::Create,
            ..
        })
    ));
    assert!(!app.state.nodes[idx].selected);
}

#[test]
fn planned_dest_not_written_until_apply() {
    let root = TempRoot::create("planned_not_written");
    fs::create_dir_all(root.join("scripts")).expect("create scripts");
    fs::write(root.join("scripts/setup.sh"), "#!/bin/sh").expect("write script");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("scripts"))
        .expect("find scripts");
    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("expand");
    let file_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("scripts/setup.sh"))
        .expect("find setup.sh");
    app.state.list_state.select(Some(file_idx));

    let node = find_node(&app.state.tree, Path::new("scripts/setup.sh")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert!(dest.is_none(), "dest must stay None until apply"),
        NodeKind::Folder { .. } => panic!("expected file"),
    }

    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("open plan");
    let node = find_node(&app.state.tree, Path::new("scripts/setup.sh")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert!(dest.is_none(), "opening Plan must not write dest"),
        NodeKind::Folder { .. } => panic!("expected file"),
    }

    let lines = collect_preview_lines(&app.state, BulkAction::Create);
    assert!(
        lines.iter().any(|l| l.contains("[fallback: .linked]")),
        "nested leftover should label .linked fallback: {lines:?}"
    );

    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("apply");
    let expected = root.join(".linked/scripts/setup.sh");
    assert!(
        fs::symlink_metadata(&expected)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "apply should create fallback dest"
    );
    let node = find_node(&app.state.tree, Path::new("scripts/setup.sh")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert_eq!(dest.as_ref(), Some(&expected)),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
}

#[test]
fn remove_unassigned_highlight_is_noop() {
    let root = TempRoot::create("remove_no_dest");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("find bashrc");
    app.state.list_state.select(Some(idx));

    let lines = collect_preview_lines(&app.state, BulkAction::Remove);
    assert!(
        lines.iter().any(|l| l.contains("REMOVE (no dest)")),
        "preview must not invent a dest: {lines:?}"
    );

    let _ = handle_key(&mut app.state, key(KeyCode::Char('x'))).expect("open remove plan");
    assert!(matches!(
        app.state.modal,
        Some(Modal::Plan {
            action: BulkAction::Remove,
            ..
        })
    ));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm remove");

    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("node");
    match &node.kind {
        NodeKind::File { dest } => assert!(dest.is_none()),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
    assert!(
        !root.join(".linked").exists(),
        "remove must not create a guessed .linked dest"
    );
}

#[test]
fn action_targets_prefers_checkboxes_over_highlight() {
    let root = TempRoot::create("action_targets");
    fs::write(root.join(".bashrc"), "a").expect("write bashrc");
    fs::write(root.join(".zshrc"), "b").expect("write zshrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let bash_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("bashrc");
    let zsh_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".zshrc"))
        .expect("zshrc");
    app.state.list_state.select(Some(zsh_idx));
    assert_eq!(
        action_targets(&app.state),
        vec![PathBuf::from(".zshrc")],
        "highlight is enough when nothing is checked"
    );

    app.state.list_state.select(Some(bash_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("check bashrc");
    app.state.list_state.select(Some(zsh_idx));
    let targets = action_targets(&app.state);
    assert_eq!(targets, vec![PathBuf::from(".bashrc")]);
    assert!(
        !app.state
            .nodes
            .iter()
            .find(|n| n.path == Path::new(".zshrc"))
            .expect("zsh")
            .selected
    );
}

#[test]
fn plan_lists_every_dir_overwrite_without_truncation() {
    let root = TempRoot::create("plan_dir_listing");
    let dest_root = root.join("existing");
    fs::create_dir_all(&dest_root).expect("dest root");

    let dir_names = [
        "dir_a", "dir_b", "dir_c", "dir_d", "dir_e", "dir_f", "dir_g", "dir_h", "dir_i",
    ];
    for name in dir_names {
        fs::create_dir_all(root.join(name)).expect("src dir");
        fs::create_dir_all(dest_root.join(name)).expect("real dest dir");
    }
    fs::write(root.join("file_z"), "src").expect("src file");
    fs::write(dest_root.join("file_z"), "existing").expect("real dest file");

    let mut app = App::new_with_root(&root).expect("app init");
    for name in dir_names.iter().chain(["file_z"].iter()) {
        assign_destination(&mut app.state, Path::new(name), dest_root.join(name))
            .expect("assign dest");
    }
    for node in app.state.nodes.iter_mut() {
        node.selected = true;
    }

    let lines = collect_preview_lines(&app.state, BulkAction::Create);
    let dir_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.contains("[OVERWRITE REAL DIR]"))
        .collect();
    assert_eq!(
        dir_lines.len(),
        9,
        "every DIR overwrite must stay in the vec"
    );
    assert!(
        dir_lines
            .iter()
            .all(|l| lines.iter().position(|x| x == *l).unwrap() < 9),
        "DIR overwrites must sort first: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("[OVERWRITE REAL FILE]")),
        "file overwrite should still be listed: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("+") && l.contains("more")),
        "Plan lines must not summarize away entries: {lines:?}"
    );
}

#[test]
fn plan_jk_scrolls_without_cancel() {
    let root = TempRoot::create("plan_scroll");
    fs::write(root.join(".a"), "a").expect("write a");
    fs::write(root.join(".b"), "b").expect("write b");

    let mut app = App::new_with_root(&root).expect("app init");
    for node in app.state.nodes.iter_mut() {
        node.selected = true;
    }
    app.state.modal = Some(Modal::Plan {
        action: BulkAction::Create,
        scroll: 0,
    });
    let _ = handle_key(&mut app.state, key(KeyCode::Char('j'))).expect("scroll down");
    match &app.state.modal {
        Some(Modal::Plan { scroll, .. }) => assert_eq!(*scroll, 1),
        other => panic!("j must not cancel Plan, got {other:?}"),
    }
    let _ = handle_key(&mut app.state, key(KeyCode::Char('k'))).expect("scroll up");
    match &app.state.modal {
        Some(Modal::Plan { scroll, .. }) => assert_eq!(*scroll, 0),
        other => panic!("k must not cancel Plan, got {other:?}"),
    }
}

#[test]
fn flatten_clears_display_children_after_badge_flag() {
    let root = TempRoot::create("flatten_shrink");
    fs::create_dir_all(root.join("keep")).expect("create keep");
    fs::write(root.join("keep/match.txt"), "m").expect("write match");

    let mut app = App::new_with_root(&root).expect("app init");
    assign_destination(
        &mut app.state,
        Path::new("keep/match.txt"),
        root.join("home/match.txt"),
    )
    .expect("assign dest");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("keep"))
        .expect("keep");
    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("expand");
    flatten_visible(&mut app.state);

    let live = find_node(&app.state.tree, Path::new("keep")).expect("live keep");
    match &live.kind {
        NodeKind::Folder { children, .. } => {
            assert!(!children.is_empty(), "live tree must keep children");
        }
        NodeKind::File { .. } => panic!("expected folder"),
    }

    let display = app
        .state
        .nodes
        .iter()
        .find(|n| n.path == Path::new("keep"))
        .expect("display keep");
    assert!(
        display.has_configured_descendant,
        "badge flag must be set from the live node"
    );
    match &display.kind {
        NodeKind::Folder { children, .. } => {
            assert!(
                children.is_empty(),
                "display clone children must be cleared"
            );
        }
        NodeKind::File { .. } => panic!("expected folder"),
    }
}

#[test]
fn source_row_zones_cover_checkbox_tree_and_planned_suffix() {
    use dd_dotstore::input::source_row_zones;
    let zones = source_row_zones("├─ ", "match.txt", Some(" → /tmp/dest"));
    assert_eq!(zones.checkbox, 0..4);
    assert_eq!(zones.icon, 4..6);
    assert_eq!(zones.mode, 6..13);
    assert_eq!(zones.tree, 13..16);
    assert!(zones.name.contains(&16), "name starts after tree");
    assert!(
        zones.name.end > zones.tree.end,
        "planned dest suffix is part of the name zone"
    );
    assert!(!zones.tree.contains(&0), "checkbox is not the tree zone");
}
