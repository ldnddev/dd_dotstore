use super::{Action, ActionMode, AppState};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::ui::toast::ToastLevel;

#[derive(Serialize, Deserialize, Default)]
pub struct PersistentData {
    pub symlinks: HashMap<String, String>,
    #[serde(default)]
    pub modes: HashMap<String, ActionMode>,
    pub history: Vec<Action>,
    /// None = key missing (1.1 configs): keep session / builtin defaults.
    /// Some(v) (including empty) = use v exactly.
    #[serde(default)]
    pub ignore_patterns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub groups: HashMap<String, String>,
}

pub const PERSIST_IDLE: Duration = Duration::from_millis(300);
pub const PERSIST_RETRY: Duration = Duration::from_secs(5);

impl AppState {
    /// Idle-since-last-mutation: every successful mutation resets the 300 ms window
    /// and drops a prior save-failure lockout so a dest-assign after fixing the
    /// path is not stuck for the rest of the 5 s. Keep last_save_error so an
    /// identical failure does not spam a new toast.
    pub fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
        self.persist_retry_at = None;
    }

    /// Snapshot maps first (so persisted_* match what save will write), then save.
    /// save() still walks state.tree; assigning persisted_* first means a later
    /// change that made save() read the maps would still be correct.
    pub fn persist_now(&mut self) -> Result<()> {
        let snap = crate::tree::snapshot_assignments(&self.tree);
        self.persisted_symlinks = snap.dests;
        self.persisted_modes = snap.modes;
        self.persisted_groups = snap.groups;
        let path = self.config_path.clone();
        save(self, &path)?;
        self.dirty_since = None;
        self.persist_retry_at = None;
        self.last_save_error = None;
        Ok(())
    }

    pub fn persist_now_if_dirty(&mut self) -> Result<()> {
        if self.dirty_since.is_some() {
            self.persist_now()
        } else {
            Ok(())
        }
    }

    /// Immediate flush from a key/mouse handler. Never returns Err — a disk
    /// error must not become handle_key `?` and tear down the TUI.
    pub fn persist_now_or_toast(&mut self) {
        if let Err(err) = self.persist_now() {
            self.record_persist_error(&err);
        }
    }

    pub fn record_persist_error(&mut self, err: &anyhow::Error) {
        let msg = format!("Save failed: {err}");
        self.persist_retry_at = Some(Instant::now() + PERSIST_RETRY);
        // leave dirty_since set; do not refresh an identical toast
        if self.last_save_error.as_deref() != Some(msg.as_str()) {
            self.show_toast(ToastLevel::Error, msg.clone());
            self.last_save_error = Some(msg);
        }
    }
}

pub fn load_persistent(path: &Path) -> Result<PersistentData> {
    let content = std::fs::read_to_string(path).context("Failed to read config")?;
    serde_json::from_str(&content).context("Failed to parse config")
}

pub fn load(path: &Path) -> Result<AppState> {
    let data = load_persistent(path)?;
    let root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut state = AppState::default_with_root(root);
    state.history = data.history.into_iter().collect();
    state.persisted_symlinks = data.symlinks;
    state.persisted_modes = data.modes;
    state.persisted_groups = data.groups;
    if let Some(pats) = data.ignore_patterns {
        state.ignore_patterns = pats;
    }
    Ok(state)
}

pub fn save(state: &AppState, path: &Path) -> Result<()> {
    let snap = crate::tree::collect_assignments(&state.tree);
    let data = PersistentData {
        symlinks: snap.dests,
        modes: snap.modes,
        history: state.history.iter().cloned().collect(),
        ignore_patterns: Some(state.ignore_patterns.clone()),
        groups: snap.groups,
    };

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(path, json)?;
    Ok(())
}
