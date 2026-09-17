mod common;

use common::{TempRoot, key};
use crossterm::event::KeyCode;
use dd_dotstore::actions::{
    apply_adopt_candidates, assign_destination, collect_preview_lines, open_doctor,
    open_group_editor,
};
use dd_dotstore::app::App;
use dd_dotstore::domain::{
    ActionMode, AdoptCandidate, BulkAction, DoctorKind, DoctorSeverity, Modal, NodeKind,
    load_persistent,
};
use dd_dotstore::inputs::handle_key;
use dd_dotstore::scan::{collect_doctor_findings, scan_symlinks_into_project};
use dd_dotstore::tree::{find_node, flatten_visible};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn group_editor_sets_and_clears_group() {
    let root = TempRoot::create("group_set");
    fs::write(root.join(".bashrc"), "export EDITOR=nvim").expect("write");

    let mut app = App::new_with_root(&root).expect("app");
    app.state.list_state.select(Some(0));
    open_group_editor(&mut app.state);
    match &app.state.modal {
        Some(Modal::GroupEditor { draft, .. }) => assert!(draft.is_empty()),
        other => panic!("expected GroupEditor, got {other:?}"),
    }

    let _ = handle_key(&mut app.state, key(KeyCode::Char('n'))).expect("n");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('v'))).expect("v");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('i'))).expect("i");
    let _ = handle_key(&mut app.state, key(KeyCode::Char('m'))).expect("m");
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("enter");

    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("node");
    assert_eq!(node.group.as_deref(), Some("nvim"));
    assert!(app.state.modal.is_none());

    app.save().expect("save");
    let data = load_persistent(&app.state.config_path).expect("load");
    assert_eq!(data.groups.get(".bashrc").map(String::as_str), Some("nvim"));

    app.state.list_state.select(Some(0));
    open_group_editor(&mut app.state);
    let _ = handle_key(&mut app.state, key(KeyCode::Backspace)).expect("bs");
    let _ = handle_key(&mut app.state, key(KeyCode::Backspace)).expect("bs");
    let _ = handle_key(&mut app.state, key(KeyCode::Backspace)).expect("bs");
    let _ = handle_key(&mut app.state, key(KeyCode::Backspace)).expect("bs");
    let _ = handle_key(&mut app.state, key(KeyCode::Enter)).expect("clear");
    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("node");
    assert_eq!(node.group, None);
}

#[test]
fn t_selects_group_and_filter_matches_group_name() {
    let root = TempRoot::create("group_select");
    fs::write(root.join(".bashrc"), "a").expect("bashrc");
    fs::write(root.join(".zshrc"), "b").expect("zshrc");
    fs::write(root.join("README"), "c").expect("readme");

    let mut app = App::new_with_root(&root).expect("app");
    for name in [".bashrc", ".zshrc"] {
        let node = find_node(&app.state.tree, Path::new(name)).expect("node");
        let path = node.path.clone();
        if let Some(n) = dd_dotstore::tree::find_mut_node(&mut app.state.tree, &path) {
            n.group = Some("shell".into());
        }
    }
    flatten_visible(&mut app.state);

    let idx = app
        .state
        .nodes
        .iter()
        .position(|n| n.path == Path::new(".bashrc"))
        .expect("idx");
    app.state.list_state.select(Some(idx));
    let _ = handle_key(&mut app.state, key(KeyCode::Char('T'))).expect("T");

    let bash = find_node(&app.state.tree, Path::new(".bashrc")).expect("bash");
    let zsh = find_node(&app.state.tree, Path::new(".zshrc")).expect("zsh");
    let readme = find_node(&app.state.tree, Path::new("README")).expect("readme");
    assert!(bash.selected);
    assert!(zsh.selected);
    assert!(!readme.selected);

    app.state.filter = "shell".into();
    flatten_visible(&mut app.state);
    assert!(
        app.state
            .nodes
            .iter()
            .any(|n| n.path == Path::new(".bashrc"))
    );
    assert!(
        app.state
            .nodes
            .iter()
            .any(|n| n.path == Path::new(".zshrc"))
    );
    assert!(
        !app.state
            .nodes
            .iter()
            .any(|n| n.path == Path::new("README"))
    );

    let lines = collect_preview_lines(&app.state, BulkAction::Create);
    assert!(lines.iter().any(|l| l.contains("[shell]")));
}

#[test]
fn import_restores_groups() {
    let root = TempRoot::create("import_groups");
    fs::write(root.join(".toolrc"), "x").expect("write");

    let mut app = App::new_with_root(&root).expect("app");
    let import_path = root.join("import.json");
    fs::write(
        &import_path,
        r#"{"symlinks":{".toolrc":"/tmp/dest_toolrc"},"modes":{},"history":[],"groups":{".toolrc":"tools"}}"#,
    )
    .expect("write import");

    dd_dotstore::actions::import_from_path(&mut app.state, &import_path).expect("import");
    let node = find_node(&app.state.tree, Path::new(".toolrc")).expect("node");
    assert_eq!(node.group.as_deref(), Some("tools"));
}

#[test]
fn import_without_groups_key_stays_valid() {
    let root = TempRoot::create("import_no_groups");
    fs::write(root.join(".toolrc"), "x").expect("write");

    let mut app = App::new_with_root(&root).expect("app");
    let import_path = root.join("import.json");
    fs::write(
        &import_path,
        r#"{"symlinks":{".toolrc":"/tmp/dest_toolrc"},"history":[]}"#,
    )
    .expect("write import");

    dd_dotstore::actions::import_from_path(&mut app.state, &import_path).expect("import");
    let node = find_node(&app.state.tree, Path::new(".toolrc")).expect("node");
    assert_eq!(node.group, None);
}

#[cfg(unix)]
#[test]
fn adopt_records_existing_project_symlink() {
    let project = TempRoot::create("adopt_proj");
    let home = TempRoot::create("adopt_home");
    fs::write(project.join(".bashrc"), "export EDITOR=nvim").expect("src");
    let dest = home.join(".bashrc");
    std::os::unix::fs::symlink(project.join(".bashrc"), &dest).expect("link");

    let found = scan_symlinks_into_project(std::slice::from_ref(&*home), &project);
    assert_eq!(found.len(), 1);

    let mut app = App::new_with_root(&project).expect("app");
    apply_adopt_candidates(
        &mut app.state,
        &[AdoptCandidate {
            dest: dest.clone(),
            src: PathBuf::from(".bashrc"),
            in_tree: true,
        }],
    );

    let node = find_node(&app.state.tree, Path::new(".bashrc")).expect("node");
    assert_eq!(node.action_mode, ActionMode::Symlink);
    match &node.kind {
        NodeKind::File { dest: node_dest } => assert_eq!(node_dest.as_ref(), Some(&dest)),
        NodeKind::Folder { .. } => panic!("expected file"),
    }
}

#[test]
fn doctor_reports_broken_and_planned() {
    let root = TempRoot::create("doctor");
    fs::write(root.join(".bashrc"), "x").expect("bashrc");
    fs::write(root.join(".zshrc"), "y").expect("zshrc");

    let mut app = App::new_with_root(&root).expect("app");
    let missing = root.join("missing-dest");
    assign_destination(&mut app.state, Path::new(".bashrc"), missing).expect("assign missing");

    let real_file = root.join("not-a-link");
    fs::write(&real_file, "nope").expect("real dest");
    assign_destination(&mut app.state, Path::new(".zshrc"), real_file).expect("assign real");

    let findings = collect_doctor_findings(&app.state);
    assert!(
        findings
            .iter()
            .any(|f| f.kind == DoctorKind::Planned && f.severity == DoctorSeverity::Warning)
    );
    assert!(
        findings
            .iter()
            .any(|f| f.kind == DoctorKind::Broken && f.severity == DoctorSeverity::Critical)
    );

    open_doctor(&mut app.state);
    assert!(matches!(app.state.modal, Some(Modal::Doctor { .. })));
}
