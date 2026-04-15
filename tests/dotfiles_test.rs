use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_dotstore::actions::{
    assign_destination, create_copy, create_symlink, push_history, remove_symlink, undo_last,
};
use dd_dotstore::app::App;
use dd_dotstore::inputs::handle_key;
use dd_dotstore::state::{
    Action, ActionMode, BrowserState, DirEntry, Modal, NodeKind, ThemeSource, load_theme,
};
use dd_dotstore::tree::{build_tree, find_node, flatten_visible};
use ratatui::style::Color;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
fn malformed_import_shows_error_modal_instead_of_failing_key_handler() {
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
    assert!(matches!(app.state.modal, Some(Modal::Error { .. })));

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
    assert!(matches!(app.state.modal, Some(Modal::Error { .. })));
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
