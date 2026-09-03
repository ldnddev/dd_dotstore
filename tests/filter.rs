mod common;

use common::{TempRoot, key};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use dd_dotstore::actions::assign_destination;
use dd_dotstore::app::App;
use dd_dotstore::inputs::handle_key;
use dd_dotstore::state::{Modal, NodeKind, load_persistent};
use dd_dotstore::tree::{find_node, flatten_visible};
use std::fs;
use std::path::{Path, PathBuf};

fn node_paths(state: &dd_dotstore::state::AppState) -> Vec<PathBuf> {
    state.nodes.iter().map(|n| n.path.clone()).collect()
}

#[test]
fn source_filter_uses_fuzzy_matching() {
    let root = TempRoot::create("source_fuzzy");
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
}

#[test]
fn large_tree_filter_handles_many_entries() {
    let root = TempRoot::create("large_tree");
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
}

#[test]
fn slash_does_not_clear_existing_filter() {
    let root = TempRoot::create("slash_keeps_filter");
    fs::write(root.join("alpha"), "a").expect("write alpha");

    let mut app = App::new_with_root(&root).expect("app init");
    app.state.filter = "alpha".to_string();
    flatten_visible(&mut app.state);

    let quit = handle_key(&mut app.state, key(KeyCode::Char('/'))).expect("slash");
    assert!(!quit);
    assert!(app.state.filter_editing);
    assert_eq!(app.state.filter, "alpha");
    assert_eq!(app.state.filter_snapshot, "alpha");
    assert!(app.state.modal.is_none());
}

#[test]
fn esc_restore_filter_flattens() {
    let root = TempRoot::create("esc_restore_filter");
    fs::create_dir_all(root.join("keep")).expect("create keep");
    fs::create_dir_all(root.join("drop")).expect("create drop");
    fs::write(root.join("keep/match.txt"), "m").expect("write match");
    fs::write(root.join("drop/nope.txt"), "n").expect("write nope");

    let mut app = App::new_with_root(&root).expect("app init");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('/'))).expect("focus filter");
    assert!(app.state.filter_snapshot.is_empty());
    for c in ['m', 'a', 't', 'c', 'h'] {
        let _ = handle_key(&mut app.state, key(KeyCode::Char(c))).expect("type");
    }
    assert_eq!(app.state.filter, "match");
    let filtered = node_paths(&app.state);
    assert!(filtered.contains(&PathBuf::from("keep")));
    assert!(filtered.contains(&PathBuf::from("keep/match.txt")));
    assert!(
        !filtered.contains(&PathBuf::from("drop")),
        "filtered view must hide unmatched folders: {filtered:?}"
    );

    let quit = handle_key(&mut app.state, key(KeyCode::Esc)).expect("esc restore");
    assert!(!quit, "Esc while editing must not quit");
    assert!(!app.state.filter_editing);
    assert!(app.state.filter.is_empty(), "snapshot was empty");
    let restored = node_paths(&app.state);
    assert!(
        restored.contains(&PathBuf::from("drop")),
        "Esc restore of empty snapshot must show the unfiltered tree, not the previous match set: {restored:?}"
    );
    assert!(restored.contains(&PathBuf::from("keep")));
}

#[test]
fn q_inserts_while_filter_editing() {
    let root = TempRoot::create("q_inserts_filter");
    fs::write(root.join("alpha"), "a").expect("write alpha");

    let mut app = App::new_with_root(&root).expect("app init");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('/'))).expect("focus filter");
    let quit = handle_key(&mut app.state, key(KeyCode::Char('q'))).expect("q insert");
    assert!(!quit, "q while filter editing must not quit");
    assert!(app.state.filter.contains('q'));
    assert!(app.state.filter_editing);
}

#[test]
fn ctrl_g_does_not_insert_while_filter_editing() {
    let root = TempRoot::create("ctrl_g_filter");
    fs::write(root.join("alpha"), "a").expect("write alpha");

    let mut app = App::new_with_root(&root).expect("app init");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('/'))).expect("focus filter");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('a'))).expect("type a");
    let before = app.state.filter.clone();
    let quit = handle_key(
        &mut app.state,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
    )
    .expect("ctrl+g");
    assert!(!quit);
    assert_eq!(app.state.filter, before);
    assert!(app.state.filter_editing);
}

#[test]
fn nested_filter_auto_expands_and_hides_unmatched_folders() {
    let root = TempRoot::create("nested_filter");
    fs::create_dir_all(root.join("keep")).expect("create keep");
    fs::create_dir_all(root.join("drop")).expect("create drop");
    fs::write(root.join("keep/match.txt"), "m").expect("write match");
    fs::write(root.join("drop/nope.txt"), "n").expect("write nope");

    let mut app = App::new_with_root(&root).expect("app init");
    let empty = node_paths(&app.state);
    assert!(empty.contains(&PathBuf::from("keep")));
    assert!(empty.contains(&PathBuf::from("drop")));
    assert!(
        !empty.contains(&PathBuf::from("keep/match.txt")),
        "collapsed folder must hide children when filter is empty: {empty:?}"
    );

    app.state.filter = "match".to_string();
    flatten_visible(&mut app.state);

    let paths = node_paths(&app.state);
    let keep_idx = paths
        .iter()
        .position(|p| p == Path::new("keep"))
        .expect("keep/ visible");
    let child_idx = paths
        .iter()
        .position(|p| p == Path::new("keep/match.txt"))
        .expect("match.txt visible");
    assert!(
        keep_idx < child_idx,
        "parent row must emit before its matching child (pre-order)"
    );
    assert!(
        !paths.iter().any(|p| p == Path::new("drop")),
        "unmatched empty folder must disappear: {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p == Path::new("drop/nope.txt")),
        "unmatched nested file must disappear: {paths:?}"
    );

    let keep = find_node(&app.state.tree, Path::new("keep")).expect("keep node");
    match &keep.kind {
        NodeKind::Folder { expanded, .. } => assert!(*expanded, "match ancestor must auto-expand"),
        NodeKind::File { .. } => panic!("expected folder"),
    }

    app.state.filter.clear();
    flatten_visible(&mut app.state);
    let after_clear = node_paths(&app.state);
    assert!(
        after_clear.contains(&PathBuf::from("drop")),
        "clearing the filter must not hide unmatched folders"
    );
    assert!(
        after_clear.contains(&PathBuf::from("keep/match.txt")),
        "clearing the filter must not auto-collapse: {after_clear:?}"
    );
}

#[test]
fn ignore_editor_add_delete_rebuilds_and_persists() {
    let root = TempRoot::create("ignore_editor");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");
    fs::write(root.join("tmpfile"), "x").expect("write tmpfile");

    let mut app = App::new_with_root(&root).expect("app init");
    let dest = root.join("home/.bashrc");
    assign_destination(&mut app.state, Path::new(".bashrc"), dest.clone()).expect("assign dest");
    assert!(find_node(&app.state.tree, Path::new("tmpfile")).is_some());

    let _ = handle_key(&mut app.state, key(KeyCode::Char('I'))).expect("open ignore editor");
    for c in ['t', 'm', 'p', 'f', 'i', 'l', 'e'] {
        let _ = handle_key(&mut app.state, key(KeyCode::Char(c))).expect("type draft");
    }
    match &app.state.modal {
        Some(Modal::IgnoreEditor { draft, .. }) => assert_eq!(draft, "tmpfile"),
        other => panic!("expected IgnoreEditor, got {other:?}"),
    }
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("add pattern");
    assert!(find_node(&app.state.tree, Path::new("tmpfile")).is_none());
    let bashrc = find_node(&app.state.tree, Path::new(".bashrc")).expect("bashrc survives");
    match &bashrc.kind {
        NodeKind::File { dest: node_dest } => assert_eq!(node_dest.as_ref(), Some(&dest)),
        NodeKind::Folder { .. } => panic!("expected file"),
    }

    let data = load_persistent(&app.state.config_path).expect("load after add");
    assert!(
        data.ignore_patterns
            .as_ref()
            .is_some_and(|p| p.iter().any(|s| s == "tmpfile")),
        "add must persist ignore_patterns immediately"
    );

    let _ = handle_key(&mut app.state, key(KeyCode::Char('d'))).expect("delete pattern");
    assert!(
        find_node(&app.state.tree, Path::new("tmpfile")).is_some(),
        "deleting the pattern must rebuild the live tree"
    );
    let data = load_persistent(&app.state.config_path).expect("load after delete");
    assert!(
        data.ignore_patterns
            .as_ref()
            .is_some_and(|p| !p.iter().any(|s| s == "tmpfile")),
        "delete must persist immediately"
    );
}

#[test]
fn ignore_editor_types_d_into_draft_without_deleting() {
    let root = TempRoot::create("ignore_editor_type_d");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write bashrc");

    let mut app = App::new_with_root(&root).expect("app init");
    let before = app.state.ignore_patterns.clone();
    assert!(before.first().is_some_and(|p| p == ".git"));

    let _ = handle_key(&mut app.state, key(KeyCode::Char('I'))).expect("open ignore editor");
    for c in "node_modules".chars() {
        let _ = handle_key(&mut app.state, key(KeyCode::Char(c))).expect("type draft");
    }
    match &app.state.modal {
        Some(Modal::IgnoreEditor { draft, selected }) => {
            assert_eq!(draft, "node_modules");
            assert_eq!(*selected, 0, "j/k while typing must not move the list");
        }
        other => panic!("expected IgnoreEditor, got {other:?}"),
    }
    assert_eq!(
        app.state.ignore_patterns, before,
        "typing d in node_modules must not delete the highlighted pattern"
    );
}
