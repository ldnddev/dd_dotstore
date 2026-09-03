use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const HISTORY_CAP: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Create { src: PathBuf, dest: PathBuf },
    Remove { src: PathBuf, dest: PathBuf },
    Copy { src: PathBuf, dest: PathBuf },
    RemoveCopy { src: PathBuf, dest: PathBuf },
}

#[derive(Clone, Copy, Debug)]
pub enum BulkAction {
    Create,
    Remove,
}

#[derive(Clone, Debug)]
pub struct Conflict {
    pub dest: PathBuf,
    pub is_real_file: bool,
    pub is_dir: bool, // true => confirm_bulk_with_overwrite will remove_dir_all
}
