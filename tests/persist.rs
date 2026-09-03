mod common;

use common::{TempRoot, broken_config_path, key};
use crossterm::event::KeyCode;
use dd_dotstore::actions::{assign_destination, create_symlink, import_from_path};
use dd_dotstore::app::App;
use dd_dotstore::inputs::handle_key;
use dd_dotstore::state::{ActionMode, NodeKind, load_persistent};
use dd_dotstore::toast::ToastLevel;
use dd_dotstore::tree::find_node;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[test]
fn persistence_round_trip_restores_destinations() {
    let root = TempRoot::create("persist");
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
}

#[test]
fn action_mode_persists_across_reload() {
    let root = TempRoot::create("mode_persist");
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
}

#[test]
fn reload_keeps_dests_and_modes_and_resets_expand() {
    let root = TempRoot::create("reload_keeps");
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
}

#[test]
fn import_restores_modes() {
    let root = TempRoot::create("import_modes");
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
}

#[test]
fn save_omits_empty_dests_and_default_modes() {
    let root = TempRoot::create("snapshot_rules");
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
}

#[test]
fn persist_tick_writes_after_idle_without_sleep() {
    let root = TempRoot::create("persist_debounce");
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
}

#[test]
fn persist_failure_toasts_once_and_retries_after_backoff() {
    let root = TempRoot::create("persist_fail");
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
}

#[test]
fn handle_key_persist_failure_returns_ok() {
    let root = TempRoot::create("key_persist_fail");
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
}

#[test]
fn dest_lock_does_not_mark_dirty() {
    let root = TempRoot::create("dest_lock_dirty");
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
}

#[test]
fn import_without_ignore_key_does_not_clobber_session_ignores() {
    let root = TempRoot::create("import_ignores_missing");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.ignore_patterns.push("session_only".into());
    let session = app.state.ignore_patterns.clone();

    let import_path = root.join("import.json");
    fs::write(&import_path, r#"{"symlinks":{},"history":[]}"#).expect("write 1.1 import");

    import_from_path(&mut app.state, &import_path).expect("import");
    assert_eq!(
        app.state.ignore_patterns, session,
        "1.1 JSON without ignore_patterns must leave session ignores alone"
    );
    assert!(
        app.state
            .ignore_patterns
            .iter()
            .any(|p| p == "session_only")
    );
    assert!(app.state.ignore_patterns.iter().any(|p| p == ".git"));
}

#[test]
fn import_with_ignore_key_replaces_session_ignores() {
    let root = TempRoot::create("import_ignores_present");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.ignore_patterns.push("session_only".into());

    let import_path = root.join("import.json");
    fs::write(
        &import_path,
        r#"{"symlinks":{},"history":[],"ignore_patterns":["only_this"]}"#,
    )
    .expect("write import with ignores");

    import_from_path(&mut app.state, &import_path).expect("import");
    assert_eq!(app.state.ignore_patterns, vec!["only_this".to_string()]);
}

#[test]
fn load_without_ignore_key_keeps_builtin_defaults() {
    let root = TempRoot::create("load_ignores_missing");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");
    fs::write(
        root.join(".dd_dotstore.json"),
        r#"{"symlinks":{},"history":[]}"#,
    )
    .expect("write 1.1 config");

    let app = App::new_with_root(&root).expect("app init");
    assert!(app.state.ignore_patterns.iter().any(|p| p == ".git"));
    assert!(app.state.ignore_patterns.iter().any(|p| p == ".linked"));
    assert!(
        app.state
            .ignore_patterns
            .iter()
            .any(|p| p == ".dd_dotstore.json")
    );
}
