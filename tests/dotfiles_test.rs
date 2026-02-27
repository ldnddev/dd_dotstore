use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_dotstore::actions::{
    assign_destination, create_symlink, push_history, remove_symlink, undo_last,
};
use dd_dotstore::app::App;
use dd_dotstore::inputs::handle_key;
use dd_dotstore::state::{Action, BrowserState, DirEntry, Modal, NodeKind};
use dd_dotstore::tree::{build_tree, find_node};
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
