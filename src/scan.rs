use crate::domain::{
    AdoptCandidate, AppState, DoctorFinding, DoctorKind, DoctorSeverity, Node, NodeKind,
    SymlinkStatus,
};
use crate::tree::find_node;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_VISITS: usize = 15_000;
const SKIP_DIR_NAMES: &[&str] = &[
    ".cache",
    ".cargo",
    ".git",
    ".mozilla",
    ".npm",
    ".rustup",
    ".steam",
    ".thunderbird",
    "Steam",
    "Trash",
    "__pycache__",
    "node_modules",
    "target",
];

pub fn default_scan_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(home) = home.as_ref() {
        roots.push(home.clone());
        let xdg_config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        roots.push(xdg_config);
        let xdg_data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));
        roots.push(xdg_data);
        roots.push(home.join(".local").join("bin"));
    }
    roots.sort();
    roots.dedup();
    roots
}

pub fn scan_symlinks_into_project(roots: &[PathBuf], project_root: &Path) -> Vec<AdoptCandidate> {
    let project_root = fs::canonicalize(project_root).unwrap_or_else(|_| normalize(project_root));
    let mut out = Vec::new();
    let mut seen_dest = HashSet::new();
    let mut visits = 0usize;
    for root in roots {
        if !root.exists() {
            continue;
        }
        let max_depth = scan_depth(root);
        walk(
            root,
            &project_root,
            0,
            max_depth,
            &mut visits,
            &mut seen_dest,
            &mut out,
        );
    }
    out.sort_by(|a, b| a.src.cmp(&b.src).then_with(|| a.dest.cmp(&b.dest)));
    out
}

fn scan_depth(root: &Path) -> usize {
    let name = root.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name == "bin" {
        1
    } else if name == "share" {
        4
    } else if name == ".config" || root.ends_with(".config") {
        8
    } else {
        1
    }
}

fn walk(
    dir: &Path,
    project_root: &Path,
    depth: usize,
    max_depth: usize,
    visits: &mut usize,
    seen_dest: &mut HashSet<PathBuf>,
    out: &mut Vec<AdoptCandidate>,
) {
    if *visits >= MAX_VISITS || depth > max_depth {
        return;
    }
    *visits += 1;
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *visits >= MAX_VISITS {
            break;
        }
        *visits += 1;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            if let Some(src) = rel_src_if_points_at_project(&path, project_root)
                && seen_dest.insert(path.clone())
            {
                let in_tree = project_root.join(&src).exists();
                out.push(AdoptCandidate {
                    dest: path,
                    src,
                    in_tree,
                });
            }
            continue;
        }
        if meta.is_dir() && depth < max_depth && !SKIP_DIR_NAMES.iter().any(|skip| name == *skip) {
            walk(
                &path,
                project_root,
                depth + 1,
                max_depth,
                visits,
                seen_dest,
                out,
            );
        }
    }
}

fn rel_src_if_points_at_project(link: &Path, project_root: &Path) -> Option<PathBuf> {
    let raw = fs::read_link(link).ok()?;
    let abs = if raw.is_absolute() {
        raw
    } else {
        link.parent()?.join(raw)
    };
    let abs = fs::canonicalize(&abs).unwrap_or_else(|_| normalize(&abs));
    abs.strip_prefix(project_root).ok().map(Path::to_path_buf)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn collect_doctor_findings(state: &AppState) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();
    walk_doctor_tree(&state.tree, &mut findings);

    let assigned_dests: HashSet<PathBuf> = assigned_dest_set(&state.tree);
    let roots = default_scan_roots();
    for candidate in scan_symlinks_into_project(&roots, &state.project_root) {
        if assigned_dests.contains(&candidate.dest) {
            continue;
        }
        findings.push(DoctorFinding {
            severity: DoctorSeverity::Info,
            kind: DoctorKind::Orphan,
            src: Some(candidate.src),
            dest: candidate.dest,
            detail: if candidate.in_tree {
                "symlink into project, not in config".to_string()
            } else {
                "symlink into project (source missing from tree)".to_string()
            },
        });
    }

    findings.sort_by(|a, b| {
        severity_rank(a.severity)
            .cmp(&severity_rank(b.severity))
            .then_with(|| a.dest.cmp(&b.dest))
    });
    findings
}

fn severity_rank(severity: DoctorSeverity) -> u8 {
    match severity {
        DoctorSeverity::Critical => 0,
        DoctorSeverity::Warning => 1,
        DoctorSeverity::Info => 2,
    }
}

fn walk_doctor_tree(nodes: &[Node], out: &mut Vec<DoctorFinding>) {
    for node in nodes {
        if let Some(dest) = node_dest(node) {
            match node.symlink_status {
                SymlinkStatus::Broken => out.push(DoctorFinding {
                    severity: DoctorSeverity::Critical,
                    kind: DoctorKind::Broken,
                    src: Some(node.path.clone()),
                    dest: dest.to_path_buf(),
                    detail: "broken or drifted dest".to_string(),
                }),
                SymlinkStatus::Planned => out.push(DoctorFinding {
                    severity: DoctorSeverity::Warning,
                    kind: DoctorKind::Planned,
                    src: Some(node.path.clone()),
                    dest: dest.to_path_buf(),
                    detail: "assigned dest is not on disk".to_string(),
                }),
                SymlinkStatus::Unknown => out.push(DoctorFinding {
                    severity: DoctorSeverity::Warning,
                    kind: DoctorKind::Unknown,
                    src: Some(node.path.clone()),
                    dest: dest.to_path_buf(),
                    detail: "could not read dest metadata".to_string(),
                }),
                SymlinkStatus::Valid | SymlinkStatus::None => {}
            }
        }
        if let NodeKind::Folder { children, .. } = &node.kind {
            walk_doctor_tree(children, out);
        }
    }
}

fn node_dest(node: &Node) -> Option<&Path> {
    match &node.kind {
        NodeKind::File { dest } | NodeKind::Folder { dest, .. } => dest.as_deref(),
    }
}

fn assigned_dest_set(tree: &[Node]) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    fn walk(nodes: &[Node], out: &mut HashSet<PathBuf>) {
        for node in nodes {
            if let Some(dest) = node_dest(node) {
                out.insert(dest.to_path_buf());
            }
            if let NodeKind::Folder { children, .. } = &node.kind {
                walk(children, out);
            }
        }
    }
    walk(tree, &mut out);
    out
}

pub fn adopt_candidates_for_state(state: &AppState) -> Vec<AdoptCandidate> {
    let assigned = assigned_dest_set(&state.tree);
    scan_symlinks_into_project(&default_scan_roots(), &state.project_root)
        .into_iter()
        .filter(|c| c.in_tree && !assigned.contains(&c.dest))
        .filter(|c| find_node(&state.tree, &c.src).is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize, rel_src_if_points_at_project, scan_symlinks_into_project};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRoot {
        path: PathBuf,
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    impl TempRoot {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "dd_dotstore_scan_{prefix}_{}_{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp");
            Self { path }
        }
    }

    #[test]
    fn normalize_drops_dot_and_parent() {
        let path = PathBuf::from("/tmp/foo/./bar/../baz");
        assert_eq!(normalize(&path), PathBuf::from("/tmp/foo/baz"));
    }

    #[cfg(unix)]
    #[test]
    fn scan_finds_symlink_into_project() {
        let project = TempRoot::new("proj");
        let home = TempRoot::new("home");
        fs::write(project.path.join(".bashrc"), "export EDITOR=nvim").expect("src");
        let dest = home.path.join(".bashrc");
        std::os::unix::fs::symlink(project.path.join(".bashrc"), &dest).expect("link");

        let found = scan_symlinks_into_project(std::slice::from_ref(&home.path), &project.path);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dest, dest);
        assert_eq!(found[0].src, PathBuf::from(".bashrc"));
        assert!(found[0].in_tree);
    }

    #[cfg(unix)]
    #[test]
    fn rel_src_rejects_outside_project() {
        let project = TempRoot::new("proj2");
        let home = TempRoot::new("home2");
        fs::write(home.path.join("notes"), "x").expect("notes");
        let dest = home.path.join("link");
        std::os::unix::fs::symlink(home.path.join("notes"), &dest).expect("link");
        assert!(rel_src_if_points_at_project(&dest, &project.path).is_none());
    }
}
