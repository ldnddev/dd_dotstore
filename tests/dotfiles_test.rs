// tests/dotfiles_test.rs

use super::*;
use std::path::Path;

#[test]
fn test_add_dotfile() {
    let mut app = App::new();
    app.add_dotfile("app".to_string(), "~/.config/app".to_string()).unwrap();
    assert_eq!(app.dotfiles[0].name, "app");
    assert_eq!(app.dotfiles[0].target, "~/.config/app");
}

#[test]
fn test_remove_dotfile() {
    let mut app = App::new();
    app.add_dotfile("app".to_string(), "~/.config/app".to_string()).unwrap();
    app.remove_dotfile(0);
    assert!(app.dotfiles.is_empty());
}

#[test]
fn test_edit_dotfile() {
    let mut app = App::new();
    app.add_dotfile("app".to_string(), "~/.config/app".to_string()).unwrap();
    app.edit_dotfile(0, "newapp".to_string(), "~/.newapp".to_string());
    assert_eq!(app.dotfiles[0].name, "newapp");
    assert_eq!(app.dotfiles[0].target, "~/.newapp");
}

#[test]
fn test_create_symlinks() {
    let app = App::new();
    app.create_symlinks("/tmp/repo").unwrap();
    // Add assertions (mock fs if needed)
}
