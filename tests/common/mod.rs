#![allow(dead_code)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TempRoot {
    pub path: PathBuf,
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl TempRoot {
    pub fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dd_dotstore_test_{prefix}_{}_{nanos}",
            std::process::id()
        ));
        Self { path }
    }

    pub fn create(prefix: &str) -> Self {
        let root = Self::new(prefix);
        fs::create_dir_all(&root.path).expect("create temp root");
        root
    }
}

impl Deref for TempRoot {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.path
    }
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub fn mouse_left(col: u16, row: u16, shift: bool) -> MouseEvent {
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

pub fn broken_config_path(root: &Path) -> PathBuf {
    let blocker = root.join("not_a_dir");
    fs::write(&blocker, "x").expect("write file-as-parent");
    blocker.join(".dd_dotstore.json")
}
