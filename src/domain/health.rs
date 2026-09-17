use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdoptCandidate {
    pub dest: PathBuf,
    pub src: PathBuf,
    pub in_tree: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorSeverity {
    Critical,
    Warning,
    Info,
}

impl DoctorSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "CRIT",
            Self::Warning => "WARN",
            Self::Info => "INFO",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorKind {
    Broken,
    Planned,
    Unknown,
    Orphan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoctorFinding {
    pub severity: DoctorSeverity,
    pub kind: DoctorKind,
    pub src: Option<PathBuf>,
    pub dest: PathBuf,
    pub detail: String,
}

impl DoctorFinding {
    pub fn line(&self) -> String {
        let src = self
            .src
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "-".to_string());
        format!(
            "{:<4}  {}  ->  {}  ({})",
            self.severity.label(),
            src,
            self.dest.display(),
            self.detail
        )
    }
}
