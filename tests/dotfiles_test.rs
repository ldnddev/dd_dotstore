use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use dd_dotstore::actions::{
    assign_destination, create_copy, create_symlink, import_from_path, push_history,
    remove_symlink, selected_create_conflicts, undo_last,
};
use dd_dotstore::app::App;
use dd_dotstore::inputs::{handle_key, handle_mouse};
use dd_dotstore::state::{
    Action, ActionMode, BrowserState, BulkAction, DirEntry, Modal, NodeKind, ThemeSource,
    load_persistent, load_theme,
};
use dd_dotstore::toast::{TOAST_DURATION, ToastLevel};
use dd_dotstore::tree::{build_tree, find_node, flatten_visible};
use dd_dotstore::ui::{overwrite_conflict_row, overwrite_warning_counts};
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos))
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn toast_expires_after_five_seconds() {
    let root = temp_path("dd_dotstore_test_toast_expiry");
    fs::create_dir_all(&root).expect("create root");

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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_tree_respects_ignore_patterns() {
    let root = temp_path("dd_dotstore_test_ignore");
    fs::create_dir_all(root.join(".git")).expect("create .git");
    fs::write(root.join(".git/config"), "x").expect("write git config");
    fs::write(root.join(".bashrc"), "alias ll='ls -la'").expect("write dotfile");

    let tree = build_tree(&root, &[".git".to_string()]);

    assert!(find_node(&tree, Path::new(".git")).is_none());
    assert!(find_node(&tree, Path::new(".bashrc")).is_some());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_initializes_with_tree_nodes() {
    let root = temp_path("dd_dotstore_test_init");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".zshrc"), "export PATH=$PATH").expect("write dotfile");

    let app = App::new_with_root(&root).expect("app init");
    assert!(!app.state.tree.is_empty());
    assert!(!app.state.nodes.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn local_standard_theme_file_is_loaded() {
    let root = temp_path("dd_dotstore_test_theme_local");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn theme_requires_schema_version() {
    let root = temp_path("dd_dotstore_test_theme_missing_version");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_falls_back_to_default_theme_when_theme_version_is_unsupported() {
    let root = temp_path("dd_dotstore_test_theme_unsupported_version");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_filter_uses_fuzzy_matching() {
    let root = temp_path("dd_dotstore_test_source_fuzzy");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("alpha.toml"), "x=1").expect("write alpha");
    fs::write(root.join("beta.conf"), "y=2").expect("write beta");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.filter = "btcf".to_string();
    flatten_visible(&mut app.state);

    assert!(
        app.state
            .nodes
            .iter()
            .any(|n| n.path == Path::new("beta.conf")),
        "expected fuzzy filter to match beta.conf",
    );
    assert!(
        !app.state
            .nodes
            .iter()
            .any(|n| n.path == Path::new("alpha.toml")),
        "expected fuzzy filter to exclude alpha.toml",
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn large_tree_filter_handles_many_entries() {
    let root = temp_path("dd_dotstore_test_large_tree");
    fs::create_dir_all(&root).expect("create root");
    for i in 0..1500 {
        fs::write(root.join(format!("file_{i:04}.conf")), "x=1").expect("write file");
    }

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.filter = "f1499".to_string();
    flatten_visible(&mut app.state);

    assert!(
        app.state
            .nodes
            .iter()
            .any(|n| n.path == Path::new("file_1499.conf"))
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_and_remove_symlink_round_trip() {
    let root = temp_path("dd_dotstore_test_links");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persistence_round_trip_restores_destinations() {
    let root = temp_path("dd_dotstore_test_persist");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".tmux.conf"), "set -g mouse on").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");
    let rel = Path::new(".tmux.conf");
    let dest = root.join("home/.tmux.conf");
    create_symlink(&mut app.state, rel, &dest).expect("create symlink");
    app.save().expect("save config");

    let reloaded = App::new_with_root(&root).expect("reload app");
    let node = find_node(&reloaded.state.tree, rel).expect("node after reload");

    match &node.kind {
        NodeKind::File { dest: node_dest } => {
            assert_eq!(node_dest.as_ref(), Some(&dest));
        }
        NodeKind::Folder { .. } => panic!("expected file node"),
    }
    assert_eq!(reloaded.state.history.len(), 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn undo_history_is_capped_and_undo_reverts_creation() {
    let root = temp_path("dd_dotstore_test_undo");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_copy_copies_file_and_undo_removes_copy() {
    let root = temp_path("dd_dotstore_test_copy_file");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bulk_copy_supports_selected_folders() {
    let root = temp_path("dd_dotstore_test_copy_folder");
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

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('m'))).expect("toggle mode");
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("select folder");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("open bulk create");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm create");

    let copied = root.join(".linked/nvim/lua/plugins.lua");
    assert_eq!(
        fs::read_to_string(copied).expect("read nested copy"),
        "return {}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn action_mode_persists_across_reload() {
    let root = temp_path("dd_dotstore_test_mode_persist");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".toolrc"), "mode=copy").expect("write source");

    let mut app = App::new_with_root(&root).expect("app init");
    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".toolrc"))
        .expect("find source");
    app.state.list_state.select(Some(idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('m'))).expect("toggle mode");
    app.save().expect("save config");

    let reloaded = App::new_with_root(&root).expect("reload app");
    let node = find_node(&reloaded.state.tree, Path::new(".toolrc")).expect("find node");
    assert_eq!(node.action_mode, ActionMode::Copy);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn modal_edit_dest_enter_selects_destination_file() {
    let root = temp_path("dd_dotstore_test_modal_dest");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn modal_import_and_export_picker_key_paths_work() {
    let root = temp_path("dd_dotstore_test_picker");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn malformed_import_shows_error_toast_instead_of_failing_key_handler() {
    let root = temp_path("dd_dotstore_test_bad_import");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn esc_closes_common_modals() {
    let root = temp_path("dd_dotstore_test_modal_esc");
    fs::create_dir_all(&root).expect("create root");
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
        Modal::Search,
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn f2_opens_and_closes_credits_modal() {
    let root = temp_path("dd_dotstore_test_credits");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".xinitrc"), "exec awesome").expect("write dotfile");

    let mut app = App::new_with_root(&root).expect("app init");
    let _ = handle_key(&mut app.state, key(KeyCode::F(2))).expect("open credits");
    assert!(matches!(app.state.modal, Some(Modal::Credits)));
    let _ = handle_key(&mut app.state, key(KeyCode::F(2))).expect("close credits");
    assert!(app.state.modal.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn modal_edit_dest_fuzzy_filter_selects_matching_entry() {
    let root = temp_path("dd_dotstore_test_modal_fuzzy");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn modal_edit_dest_supports_home_and_jump_shortcuts() {
    let root = temp_path("dd_dotstore_test_modal_shortcuts");
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
        assert_eq!(browser.current, root);
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cannot_change_destination_while_existing_symlink_is_present() {
    let root = temp_path("dd_dotstore_test_dest_lock");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bulk_create_conflict_requires_overwrite_confirmation() {
    let root = temp_path("dd_dotstore_test_overwrite_warning");
    fs::create_dir_all(&root).expect("create root");
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

    app.state.modal = Some(Modal::ConfirmBulk {
        action: dd_dotstore::state::BulkAction::Create,
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn space_toggles_folder_selection_in_main_view() {
    let root = temp_path("dd_dotstore_test_folder_select");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn hl_toggles_folder_expand_collapse() {
    let root = temp_path("dd_dotstore_test_folder_expand");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn enter_on_folder_opens_destination_editor() {
    let root = temp_path("dd_dotstore_test_folder_enter_dest");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bulk_create_and_remove_supports_selected_folders() {
    let root = temp_path("dd_dotstore_test_folder_bulk");
    fs::create_dir_all(root.join("alacritty")).expect("create alacritty dir");
    fs::write(root.join("alacritty/alacritty.toml"), "[window]").expect("write file");

    let mut app = App::new_with_root(&root).expect("app init");
    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("alacritty"))
        .expect("find folder index");
    let folder_link = root.join(".linked/alacritty");

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("select folder");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("open bulk create");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm create");

    assert!(
        fs::symlink_metadata(&folder_link)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "expected folder symlink at fallback destination"
    );

    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("reselect folder");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('x'))).expect("open bulk remove");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("confirm remove");

    assert!(
        fs::symlink_metadata(&folder_link).is_err(),
        "expected folder symlink to be removed"
    );

    let _ = fs::remove_dir_all(root);
}

fn broken_config_path(root: &Path) -> PathBuf {
    let blocker = root.join("not_a_dir");
    fs::write(&blocker, "x").expect("write file-as-parent");
    blocker.join(".dd_dotstore.json")
}

#[test]
fn reload_keeps_dests_and_modes_and_resets_expand() {
    let root = temp_path("dd_dotstore_test_reload_keeps");
    fs::create_dir_all(root.join("nvim")).expect("create nvim dir");
    fs::write(root.join("nvim/init.lua"), "return {}").expect("write nested");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let dest = root.join("home/.bashrc");
    assign_destination(&mut app.state, Path::new(".bashrc"), dest.clone()).expect("assign dest");

    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("find .bashrc");
    app.state.list_state.select(Some(idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('m'))).expect("toggle mode");

    let folder_idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new("nvim"))
        .expect("find nvim");
    app.state.list_state.select(Some(folder_idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("expand");

    app.reload().expect("reload");

    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("bashrc after reload");
    match &node.kind {
        NodeKind::File { dest: node_dest } => assert_eq!(node_dest.as_ref(), Some(&dest)),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
    assert_eq!(node.action_mode, ActionMode::Copy);

    let folder = find_node(&app.state.tree, Path::new("nvim")).expect("nvim after reload");
    match &folder.kind {
        NodeKind::Folder { expanded, .. } => assert!(!*expanded, "expand state resets on rebuild"),
        NodeKind::File { .. } => panic!("expected folder"),
    }

    let _ = handle_key(&mut app.state, key(KeyCode::Char('l'))).expect("expand again");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('r'))).expect("r reload");
    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("bashrc after r");
    assert_eq!(node.action_mode, ActionMode::Copy);
    match &node.kind {
        NodeKind::File { dest: node_dest } => assert_eq!(node_dest.as_ref(), Some(&dest)),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
    let folder = find_node(&app.state.tree, Path::new("nvim")).expect("nvim after r");
    match &folder.kind {
        NodeKind::Folder { expanded, .. } => assert!(!*expanded),
        NodeKind::File { .. } => panic!("expected folder"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_restores_modes() {
    let root = temp_path("dd_dotstore_test_import_modes");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".toolrc"), "mode=copy").expect("write source");

    let mut app = App::new_with_root(&root).expect("app init");
    let import_path = root.join("import.json");
    fs::write(
        &import_path,
        r#"{"symlinks":{".toolrc":"/tmp/dest_toolrc"},"modes":{".toolrc":"copy"},"history":[]}"#,
    )
    .expect("write import");

    import_from_path(&mut app.state, &import_path).expect("import");

    let node = find_node(&app.state.tree, Path::new(".toolrc")).expect("node");
    assert_eq!(node.action_mode, ActionMode::Copy);
    match &node.kind {
        NodeKind::File { dest } => {
            assert_eq!(dest.as_ref(), Some(&PathBuf::from("/tmp/dest_toolrc")))
        }
        NodeKind::Folder { .. } => panic!("expected file"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn save_omits_empty_dests_and_default_modes() {
    let root = temp_path("dd_dotstore_test_snapshot_rules");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");
    fs::write(root.join(".toolrc"), "mode=copy").expect("write toolrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let dest = root.join("home/.bashrc");
    assign_destination(&mut app.state, Path::new(".bashrc"), dest.clone()).expect("assign dest");

    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".toolrc"))
        .expect("find toolrc");
    app.state.list_state.select(Some(idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('m'))).expect("toggle mode");

    app.save().expect("persist");
    let data = load_persistent(&app.state.config_path).expect("load snapshot");
    assert_eq!(
        data.symlinks.get(".bashrc").map(String::as_str),
        Some(dest.to_string_lossy().as_ref())
    );
    assert!(!data.symlinks.contains_key(".toolrc"));
    assert_eq!(data.modes.get(".toolrc"), Some(&ActionMode::Copy));
    assert!(!data.modes.contains_key(".bashrc"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_tick_writes_after_idle_without_sleep() {
    let root = temp_path("dd_dotstore_test_persist_debounce");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let config = app.state.config_path.clone();
    assert!(!config.exists());

    app.state.mark_dirty();
    app.tick();
    assert!(!config.exists(), "idle window should not have elapsed");

    app.state.dirty_since = Some(Instant::now() - Duration::from_secs(1));
    app.tick();
    assert!(
        config.exists(),
        "backdated dirty_since should flush on tick"
    );
    assert!(app.state.dirty_since.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_failure_toasts_once_and_retries_after_backoff() {
    let root = temp_path("dd_dotstore_test_persist_fail");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.config_path = broken_config_path(&root);

    app.state.mark_dirty();
    app.state.dirty_since = Some(Instant::now() - Duration::from_secs(1));
    app.tick();

    let toast = app.state.toast.as_ref().expect("error toast");
    assert_eq!(toast.level, ToastLevel::Error);
    assert!(
        toast.message.starts_with("Save failed:"),
        "unexpected toast: {}",
        toast.message
    );
    let first_created = toast.created_at;
    let first_msg = toast.message.clone();
    assert!(app.state.persist_retry_at.is_some());
    assert_eq!(
        app.state.last_save_error.as_deref(),
        Some(first_msg.as_str())
    );

    app.tick();
    let toast = app.state.toast.as_ref().expect("toast after second tick");
    assert_eq!(toast.created_at, first_created);
    assert_eq!(toast.message, first_msg);

    app.state.persist_retry_at = Some(Instant::now() - Duration::from_millis(1));
    app.tick();
    let toast = app.state.toast.as_ref().expect("toast after retry");
    assert_eq!(toast.created_at, first_created);
    assert_eq!(toast.message, first_msg);
    assert!(
        app.state
            .persist_retry_at
            .is_some_and(|at| at > Instant::now()),
        "retry should reschedule backoff"
    );

    app.state.mark_dirty();
    assert!(app.state.persist_retry_at.is_none());
    assert_eq!(
        app.state.last_save_error.as_deref(),
        Some(first_msg.as_str())
    );
    app.state.dirty_since = Some(Instant::now() - Duration::from_secs(1));
    app.tick();
    assert!(
        app.state
            .persist_retry_at
            .is_some_and(|at| at > Instant::now())
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn handle_key_persist_failure_returns_ok() {
    let root = temp_path("dd_dotstore_test_key_persist_fail");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".vimrc"), "set number").expect("write vimrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let rel = Path::new(".vimrc");
    let dest = root.join("home/.vimrc");
    create_symlink(&mut app.state, rel, &dest).expect("create symlink");
    app.state.config_path = broken_config_path(&root);

    let handled = handle_key(&mut app.state, key(KeyCode::Char('u'))).expect("undo key");
    assert!(!handled, "persist failure must not quit");
    let toast = app.state.toast.as_ref().expect("save-failed toast");
    assert_eq!(toast.level, ToastLevel::Error);
    assert!(
        toast.message.starts_with("Save failed:"),
        "unexpected toast: {}",
        toast.message
    );
    assert!(!toast.message.contains("Import failed"));

    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == rel)
        .expect("find vimrc");
    app.state.list_state.select(Some(idx));
    assign_destination(&mut app.state, rel, dest.clone()).expect("reassign dest");
    let _ = handle_key(&mut app.state, key(KeyCode::Char(' '))).expect("select");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('s'))).expect("open confirm");
    let handled = handle_key(&mut app.state, key(KeyCode::Char('y'))).expect("apply");
    assert!(!handled, "apply persist failure must not quit");
    let toast = app.state.toast.as_ref().expect("apply save-failed toast");
    assert!(toast.message.starts_with("Save failed:"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dest_lock_does_not_mark_dirty() {
    let root = temp_path("dd_dotstore_test_dest_lock_dirty");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".zprofile"), "export ZDOTDIR=$HOME").expect("write zprofile");

    let mut app = App::new_with_root(&root).expect("app init");
    let rel = Path::new(".zprofile");
    let old_dest = root.join("home/.zprofile");
    let new_dest = root.join("home/.zprofile_new");
    create_symlink(&mut app.state, rel, &old_dest).expect("create initial symlink");
    app.state.persist_now().expect("clear dirty");
    assert!(app.state.dirty_since.is_none());

    assign_destination(&mut app.state, rel, new_dest.clone()).expect("dest lock");
    assert!(
        app.state.dirty_since.is_none(),
        "dest-lock must not mark dirty"
    );

    let _ = fs::remove_dir_all(root);
}

fn mouse_left(col: u16, row: u16, shift: bool) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: if shift {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        },
    }
}

#[test]
fn shift_click_range_selects_from_previous_highlight() {
    let root = temp_path("dd_dotstore_test_shift_click");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("alpha"), "a").expect("write alpha");
    fs::write(root.join("bravo"), "b").expect("write bravo");
    fs::write(root.join("charlie"), "c").expect("write charlie");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.source_area = Rect::new(0, 0, 40, 20);
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn esc_does_not_quit_main_view() {
    let root = temp_path("dd_dotstore_test_esc_no_quit");
    fs::create_dir_all(&root).expect("create root");
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

    let _ = fs::remove_dir_all(root);
}

#[test]
#[allow(non_snake_case)]
fn q_and_Q_quit() {
    let root = temp_path("dd_dotstore_test_q_quit");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".zshrc"), "export PATH=$PATH").expect("write zshrc");

    let mut app = App::new_with_root(&root).expect("app init");
    assert!(handle_key(&mut app.state, key(KeyCode::Char('q'))).expect("q"));
    assert!(handle_key(&mut app.state, key(KeyCode::Char('Q'))).expect("Q"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn overwrite_lists_all_dirs_then_files() {
    let root = temp_path("dd_dotstore_test_overwrite_all_dirs");
    fs::create_dir_all(&root).expect("create root");
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

    app.state.modal = Some(Modal::ConfirmBulk {
        action: BulkAction::Create,
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn click_inside_confirm_overwrite_preview_is_noop() {
    let root = temp_path("dd_dotstore_test_click_inside_noop");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join(".profile"), "export PATH=$PATH").expect("write profile");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.current_modal_area = Some(Rect::new(10, 10, 40, 20));

    for modal in [
        Modal::ConfirmBulk {
            action: BulkAction::Create,
        },
        Modal::PreviewBulk {
            action: BulkAction::Create,
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
            "click inside confirm/overwrite/preview must be a no-op"
        );
    }

    app.state.modal = Some(Modal::ConfirmBulk {
        action: BulkAction::Create,
    });
    let _ = handle_mouse(&mut app.state, mouse_left(0, 0, false)).expect("outside click");
    assert!(app.state.modal.is_none(), "click outside must still cancel");

    let _ = fs::remove_dir_all(root);
}
