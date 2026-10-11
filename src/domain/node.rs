use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    #[default]
    Symlink,
    Copy,
}

impl ActionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Symlink => "LINK",
            Self::Copy => "COPY",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Self::Symlink => "linked",
            Self::Copy => "copied",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Symlink => Self::Copy,
            Self::Copy => Self::Symlink,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    File {
        dest: Option<PathBuf>,
    },
    Folder {
        children: Vec<Node>,
        expanded: bool,
        dest: Option<PathBuf>,
    },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub kind: NodeKind,
    pub selected: bool,
    pub action_mode: ActionMode,
    pub symlink_status: SymlinkStatus,
    /// Optional apply-group name (nvim, shell, …). Persisted when non-empty.
    pub group: Option<String>,
    /// Display-only. Set from the live tree at flatten; not persisted.
    pub has_configured_descendant: bool,
}

impl Node {
    /// Saved destination, if this row is in the map. `None` means the UI may
    /// still show a suggested path from the XDG / `$HOME` / `.linked` rules.
    pub fn assigned_dest(&self) -> Option<&Path> {
        match &self.kind {
            NodeKind::File { dest } | NodeKind::Folder { dest, .. } => dest.as_deref(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymlinkStatus {
    None,
    Planned, // dest assigned, path NotFound
    Valid,
    Broken,
    Unknown, // dest Some, metadata Err other than NotFound
}
