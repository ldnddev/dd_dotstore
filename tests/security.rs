mod common;

use common::TempRoot;
use dd_dotstore::actions::{create_copy, undo_last};
use dd_dotstore::app::App;
use dd_dotstore::state::NodeKind;
use dd_dotstore::tree::build_tree;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt, symlink};
use std::path::Path;
use std::process::Command;

#[test]
fn copy_preserves_symlinks_instead_of_following_them() {
    let root = TempRoot::create("copy_link");
    let outside = TempRoot::create("copy_link_out");
    fs::create_dir_all(outside.join("secret_dir")).expect("outside dir");
    fs::write(outside.join("secret_dir/id"), "PRIVATE").expect("secret");
    fs::create_dir_all(root.join("pkg")).expect("pkg");
    fs::write(root.join("pkg/plain"), "ok").expect("plain");
    symlink(outside.join("secret_dir"), root.join("pkg/keys")).expect("dir link");
    symlink(outside.join("secret_dir/id"), root.join("pkg/keyfile")).expect("file link");

    let mut app = App::new_with_root(&root).expect("app");
    let dest = root.join("copied");
    let copied = create_copy(&mut app.state, Path::new("pkg"), &dest).expect("copy");
    assert!(copied);

    let keys = fs::symlink_metadata(dest.join("keys")).expect("keys meta");
    assert!(
        keys.file_type().is_symlink(),
        "directory symlink was followed"
    );
    assert_eq!(
        fs::read_link(dest.join("keys")).expect("keys target"),
        outside.join("secret_dir")
    );
    assert!(
        fs::symlink_metadata(dest.join("keyfile"))
            .expect("keyfile meta")
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(dest.join("plain")).expect("plain"), "ok");
}

#[test]
fn copy_refuses_special_files() {
    let root = TempRoot::create("copy_fifo");
    let fifo = root.join("pipe");
    let status = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(status.success());

    let mut app = App::new_with_root(&root).expect("app");
    let err = create_copy(&mut app.state, Path::new("pipe"), &root.join("out")).expect_err("fifo");
    assert!(format!("{err:#}").contains("special file"), "{err:#}");
    assert!(!root.join("out").exists());
}

#[test]
fn copy_rejects_source_that_escapes_the_project() {
    let root = TempRoot::create("copy_escape");
    fs::write(root.join("inside"), "x").expect("inside");
    let mut app = App::new_with_root(&root).expect("app");
    let err = create_copy(&mut app.state, Path::new("../outside"), &root.join("out"))
        .expect_err("escape");
    assert!(format!("{err:#}").contains("inside the project"), "{err:#}");
}

#[test]
fn tree_does_not_descend_into_symlinked_directories() {
    let root = TempRoot::create("tree_link");
    let outside = TempRoot::create("tree_link_out");
    fs::write(outside.join("secret_name"), "PRIVATE").expect("secret");
    symlink(&outside.path, root.join("linkdir")).expect("linkdir");

    let nodes = build_tree(&root, &[]);
    let link = nodes
        .iter()
        .find(|n| n.path == Path::new("linkdir"))
        .expect("linkdir node");
    assert!(
        matches!(link.kind, NodeKind::File { .. }),
        "symlink directory was walked"
    );
    assert!(nodes.iter().all(|n| n.name != "secret_name"));
}

#[test]
fn config_symlink_is_not_read_or_written() {
    let root = TempRoot::create("cfg_link");
    fs::write(root.join(".bashrc"), "export A=1").expect("src");
    let important = root.join("important.txt");
    fs::write(&important, "ORIGINAL").expect("important");
    symlink(&important, root.join(".dd_dotstore.json")).expect("config link");

    let err = match App::new_with_root(&root) {
        Ok(_) => panic!("linked config was loaded"),
        Err(err) => err,
    };
    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert_eq!(
        fs::read_to_string(&important).expect("important"),
        "ORIGINAL"
    );

    let clean = TempRoot::create("cfg_link_save");
    fs::write(clean.join(".bashrc"), "export A=1").expect("src");
    let mut app = App::new_with_root(&clean).expect("app");
    let target = clean.join("important.txt");
    fs::write(&target, "ORIGINAL").expect("target");
    let link = clean.join("linked.json");
    symlink(&target, &link).expect("link");
    app.state.config_path = link.clone();
    app.state.mark_dirty();
    let err = app.state.persist_now().expect_err("refuse save");
    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert_eq!(fs::read_to_string(&target).expect("target"), "ORIGINAL");
    assert!(
        fs::symlink_metadata(&link)
            .expect("link")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn linux_o_nofollow_does_not_open_a_symlink() {
    let root = TempRoot::create("o_nofollow");
    let target = root.join("target");
    fs::write(&target, "secret").expect("target");
    let link = root.join("link");
    symlink(&target, &link).expect("link");
    // Same Linux flags as `open_no_follow`: O_NOFOLLOW is 0x20000.
    let err = OpenOptions::new()
        .read(true)
        .custom_flags(0x20000)
        .open(&link)
        .expect_err("symlink open");
    assert_eq!(err.raw_os_error(), Some(40), "{err}");
    assert_eq!(fs::read_to_string(&target).expect("target"), "secret");
}

#[test]
fn config_fifo_is_refused_without_blocking() {
    let root = TempRoot::create("cfg_fifo");
    let fifo = root.join(".dd_dotstore.json");
    let status = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(status.success());
    let err = match App::new_with_root(&root) {
        Ok(_) => panic!("fifo config should not load"),
        Err(err) => err,
    };
    assert!(format!("{err:#}").contains("non-file"), "{err:#}");
}

#[test]
fn on_disk_undo_history_is_not_replayed() {
    let root = TempRoot::create("hist");
    fs::write(root.join(".bashrc"), "export A=1").expect("src");
    let victim = root.join("victim");
    fs::create_dir(&victim).expect("victim");
    fs::write(victim.join("secret"), "keep").expect("secret");
    let dest = victim.display().to_string();
    let config = format!(
        r#"{{"symlinks":{{}},"history":[{{"Copy":{{"src":".bashrc","dest":"{dest}"}}}}]}}"#
    );
    fs::write(root.join(".dd_dotstore.json"), config).expect("config");

    let mut app = App::new_with_root(&root).expect("app");
    assert!(app.state.history.is_empty());
    undo_last(&mut app.state).expect("undo");
    assert_eq!(
        fs::read_to_string(victim.join("secret")).expect("secret"),
        "keep"
    );
}
