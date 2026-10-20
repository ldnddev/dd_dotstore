use super::{Action, ActionMode, AppState};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::ui::toast::ToastLevel;

/// Caps a cloned project's config so a huge file cannot be read into memory.
const MAX_CONFIG_BYTES: u64 = 8 * 1024 * 1024;

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
    let content = read_no_follow(path, MAX_CONFIG_BYTES).context("Failed to read config")?;
    let mut data: PersistentData =
        serde_json::from_str(&content).context("Failed to parse config")?;
    // A project file is often cloned. Undo of a stored Copy/Remove would
    // delete or plant files the first time the user presses `u`.
    data.history.clear();
    Ok(data)
}

fn read_no_follow(path: &Path, max_bytes: u64) -> Result<String> {
    let meta =
        fs::symlink_metadata(path).with_context(|| format!("Failed to read {}", path.display()))?;
    if meta.file_type().is_symlink() {
        bail!("refusing to follow symlink {}", path.display());
    }
    if !meta.is_file() {
        bail!("refusing to read non-file {}", path.display());
    }
    if meta.len() > max_bytes {
        bail!(
            "{} is {} bytes; refusing to read more than {max_bytes}",
            path.display(),
            meta.len()
        );
    }
    let file =
        open_no_follow(path).with_context(|| format!("Failed to read {}", path.display()))?;
    // Stat the fd, not the path. A symlink or fifo swapped in after the
    // path check must not be followed or block this read.
    let opened = file
        .metadata()
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if opened.file_type().is_symlink() || !opened.is_file() {
        bail!("refusing to read non-file {}", path.display());
    }
    if opened.len() > max_bytes {
        bail!(
            "{} is {} bytes; refusing to read more than {max_bytes}",
            path.display(),
            opened.len()
        );
    }
    let mut buf = String::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_string(&mut buf)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if buf.len() as u64 > max_bytes {
        bail!("{} exceeds {max_bytes} bytes", path.display());
    }
    Ok(buf)
}

/// Open the final path component without following a symlink.
///
/// `File::open` follows a symlink swapped in after `symlink_metadata`.
/// `O_NOFOLLOW` makes that open fail. `O_NONBLOCK` keeps a swapped-in fifo
/// from blocking. Linux values: `O_NOFOLLOW` is `0x20000`, `O_NONBLOCK` is `0x800`.
fn open_no_follow(path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NOFOLLOW does not follow a final symlink. O_NONBLOCK so a fifo
        // open returns instead of waiting.
        const O_NOFOLLOW: i32 = 0x20000;
        const O_NONBLOCK: i32 = 0x800;
        options.custom_flags(O_NOFOLLOW | O_NONBLOCK);
    }
    let opened = options.open(path);
    #[cfg(target_os = "linux")]
    {
        // ELOOP. ErrorKind::FilesystemLoop is still unstable.
        const ELOOP: i32 = 40;
        if let Err(err) = &opened {
            if err.raw_os_error() == Some(ELOOP) {
                return Err(std::io::Error::new(
                    err.kind(),
                    format!("refusing to follow symlink {}", path.display()),
                ));
            }
        }
    }
    opened
}

/// Replace `path` without following a final symlink. `std::fs::write` opens
/// with truncation and would overwrite the symlink target (for example
/// `~/.bashrc` when a repo committed `.dd_dotstore.json` as a link).
fn write_no_follow(path: &Path, contents: &str) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            bail!("refusing to follow symlink {}", path.display());
        }
        if !meta.is_file() {
            bail!("refusing to overwrite non-file {}", path.display());
        }
    }

    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let tmp = parent.join(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("Failed to write {}", path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to write {}", path.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("Failed to replace {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
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
    write_no_follow(path, &json)?;
    Ok(())
}
