use crate::actions::planned_dest;
use crate::domain::{ActionMode, AppState, Node, NodeKind, SymlinkStatus};
use crate::theme::ThemeColors;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusKind {
    Suggested,
    Linked,
    Copied,
    Planned,
    Broken,
    Unknown,
}

impl FocusKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Suggested => "suggested (not saved)",
            Self::Linked => "linked",
            Self::Copied => "copied",
            Self::Planned => "planned",
            Self::Broken => "broken",
            Self::Unknown => "unknown",
        }
    }

    fn is_suggested(self) -> bool {
        matches!(self, Self::Suggested)
    }
}

pub(crate) fn focus_kind(node: &Node) -> FocusKind {
    if node.assigned_dest().is_none() {
        return FocusKind::Suggested;
    }
    match (node.symlink_status, node.action_mode) {
        (SymlinkStatus::Valid, ActionMode::Copy) => FocusKind::Copied,
        (SymlinkStatus::Valid, ActionMode::Symlink) => FocusKind::Linked,
        (SymlinkStatus::Broken, _) => FocusKind::Broken,
        (SymlinkStatus::Planned, _) => FocusKind::Planned,
        (SymlinkStatus::Unknown | SymlinkStatus::None, _) => FocusKind::Unknown,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FocusParts {
    pub mode: &'static str,
    pub source: String,
    pub group: String,
    pub dest: String,
    pub status: &'static str,
    pub kind: FocusKind,
}

impl FocusParts {
    pub(crate) fn plain(&self) -> String {
        if self.mode.is_empty() && self.status.is_empty() {
            return self.source.clone();
        }
        format!(
            "{}  {}{}  →  {}  {}",
            self.mode, self.source, self.group, self.dest, self.status
        )
    }
}

pub(crate) fn focus_parts(state: &AppState) -> Option<FocusParts> {
    let idx = state.list_state.selected()?;
    let node = state.nodes.get(idx)?;
    let kind = focus_kind(node);
    let dest = planned_dest(state, node);
    Some(FocusParts {
        mode: node.action_mode.label(),
        source: node.path.display().to_string(),
        group: node
            .group
            .as_ref()
            .map(|group| format!(" ·{group}"))
            .unwrap_or_default(),
        dest: dest.display().to_string(),
        status: kind.label(),
        kind,
    })
}

/// Fit the focus line to `width` columns. The status word stays intact unless
/// the terminal is too narrow to hold the skeleton. Destinations lose their
/// prefix first, then the source path, so the end of a long home path remains.
pub(crate) fn fit_focus(mut parts: FocusParts, width: usize) -> FocusParts {
    if width == 0 {
        return collapsed(String::new());
    }
    if chars_len(&parts.plain()) <= width {
        return parts;
    }

    let mode_prefix = chars_len(parts.mode) + 2;
    let arrow = chars_len("  →  ");
    let status_suffix = 2 + chars_len(parts.status);
    let fixed = mode_prefix + arrow + status_suffix;
    if fixed >= width {
        return collapsed(ellipsize_end(&parts.plain(), width));
    }

    let budget = width - fixed;
    let source_full = format!("{}{}", parts.source, parts.group);
    let source_len = chars_len(&source_full);
    if source_len >= budget {
        let dest_budget = if budget <= 1 {
            0
        } else {
            (budget / 3).max(1).min(budget - 1)
        };
        let source_budget = budget - dest_budget;
        parts.source = ellipsize_start(&source_full, source_budget);
        parts.group.clear();
        parts.dest = ellipsize_start(&parts.dest, dest_budget);
    } else {
        parts.dest = ellipsize_start(&parts.dest, budget - source_len);
    }

    if chars_len(&parts.plain()) > width {
        return collapsed(ellipsize_end(&parts.plain(), width));
    }
    parts
}

fn collapsed(text: String) -> FocusParts {
    FocusParts {
        mode: "",
        source: text,
        group: String::new(),
        dest: String::new(),
        status: "",
        kind: FocusKind::Unknown,
    }
}

pub(crate) fn focus_header_line(state: &AppState, width: usize) -> Line<'static> {
    let bg = state.theme.colors.base_background;
    let Some(parts) = focus_parts(state) else {
        return Line::from(Span::styled(
            ellipsize_end("No row highlighted", width),
            on_header(state.theme.colors.text_secondary, bg),
        ));
    };
    let kind = parts.kind;
    let folder = matches!(
        state
            .nodes
            .get(state.list_state.selected().unwrap_or(0))
            .map(|node| &node.kind),
        Some(NodeKind::Folder { .. })
    );
    let fitted = fit_focus(parts, width);
    if fitted.mode.is_empty() || kind.is_suggested() {
        let color = if kind.is_suggested() {
            state.theme.colors.text_secondary
        } else {
            state.theme.colors.text_primary
        };
        return Line::from(Span::styled(fitted.plain(), on_header(color, bg)));
    }
    line_from_parts(&fitted, &state.theme.colors, bg, folder)
}

fn line_from_parts(
    parts: &FocusParts,
    colors: &ThemeColors,
    bg: Color,
    folder: bool,
) -> Line<'static> {
    let mode_color = match parts.mode {
        "COPY" => colors.warning,
        _ => colors.success,
    };
    let source_color = if folder { colors.folders } else { colors.files };
    let dest_color = match parts.mode {
        "COPY" => colors.warning,
        _ => colors.links,
    };
    let status_color = match parts.kind {
        FocusKind::Linked | FocusKind::Copied => colors.success,
        FocusKind::Planned => colors.info,
        FocusKind::Broken => colors.error,
        FocusKind::Unknown | FocusKind::Suggested => colors.warning,
    };
    let mut spans = vec![
        Span::styled(format!("{}  ", parts.mode), on_header(mode_color, bg)),
        Span::styled(parts.source.clone(), on_header(source_color, bg)),
    ];
    if !parts.group.is_empty() {
        spans.push(Span::styled(
            parts.group.clone(),
            on_header(colors.text_secondary, bg),
        ));
    }
    spans.push(Span::styled("  →  ", on_header(colors.text_primary, bg)));
    spans.push(Span::styled(parts.dest.clone(), on_header(dest_color, bg)));
    spans.push(Span::styled(
        format!("  {}", parts.status),
        on_header(status_color, bg),
    ));
    Line::from(spans)
}

fn on_header(color: Color, bg: Color) -> Style {
    Style::default().fg(color).bg(bg)
}

fn chars_len(s: &str) -> usize {
    s.chars().count()
}

fn ellipsize_end(s: &str, max: usize) -> String {
    if chars_len(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

fn ellipsize_start(s: &str, max: usize) -> String {
    if chars_len(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_string();
    }
    let tail: String = s
        .chars()
        .rev()
        .take(max - 1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ActionMode, Node, NodeKind, SymlinkStatus};
    use std::path::PathBuf;

    fn node(
        path: &str,
        dest: Option<&str>,
        mode: ActionMode,
        status: SymlinkStatus,
        group: Option<&str>,
    ) -> Node {
        Node {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            path: PathBuf::from(path),
            kind: NodeKind::File {
                dest: dest.map(PathBuf::from),
            },
            selected: false,
            action_mode: mode,
            symlink_status: status,
            group: group.map(str::to_string),
            has_configured_descendant: false,
        }
    }

    fn parts_for(node: &Node, dest: &str) -> FocusParts {
        let kind = focus_kind(node);
        FocusParts {
            mode: node.action_mode.label(),
            source: node.path.display().to_string(),
            group: node
                .group
                .as_ref()
                .map(|group| format!(" ·{group}"))
                .unwrap_or_default(),
            dest: dest.to_string(),
            status: kind.label(),
            kind,
        }
    }

    #[test]
    fn status_words_follow_saved_state() {
        assert_eq!(
            focus_kind(&node(
                ".zshrc",
                Some("/home/me/.zshrc"),
                ActionMode::Symlink,
                SymlinkStatus::Valid,
                None
            )),
            FocusKind::Linked
        );
        assert_eq!(
            focus_kind(&node(
                "README.md",
                Some("/home/me/README.md"),
                ActionMode::Copy,
                SymlinkStatus::Valid,
                None
            )),
            FocusKind::Copied
        );
        assert_eq!(
            focus_kind(&node(
                ".gitconfig",
                Some("/home/me/.gitconfig"),
                ActionMode::Symlink,
                SymlinkStatus::Planned,
                None
            )),
            FocusKind::Planned
        );
        assert_eq!(
            focus_kind(&node(
                ".gitconfig",
                Some("/home/me/.gitconfig"),
                ActionMode::Copy,
                SymlinkStatus::Broken,
                None
            )),
            FocusKind::Broken
        );
        assert_eq!(
            focus_kind(&node(
                ".bashrc",
                None,
                ActionMode::Symlink,
                SymlinkStatus::None,
                None
            )),
            FocusKind::Suggested
        );
    }

    #[test]
    fn short_saved_line_keeps_group_dest_and_status() {
        let text = fit_focus(
            parts_for(
                &node(
                    ".zshrc",
                    Some("/tmp/dd-home/.zshrc"),
                    ActionMode::Symlink,
                    SymlinkStatus::Valid,
                    Some("shell"),
                ),
                "/tmp/dd-home/.zshrc",
            ),
            80,
        )
        .plain();
        assert_eq!(text, "LINK  .zshrc ·shell  →  /tmp/dd-home/.zshrc  linked");
    }

    #[test]
    fn suggested_line_is_marked_not_saved() {
        let text = fit_focus(
            parts_for(
                &node(
                    ".bashrc",
                    None,
                    ActionMode::Symlink,
                    SymlinkStatus::None,
                    None,
                ),
                "/tmp/dd-home/.bashrc",
            ),
            80,
        )
        .plain();
        assert_eq!(
            text,
            "LINK  .bashrc  →  /tmp/dd-home/.bashrc  suggested (not saved)"
        );
        assert!(!text.contains("linked"));
    }

    #[test]
    fn long_dest_keeps_the_status_word_and_the_path_tail() {
        let dest = "/tmp/dd-home/.config/alacritty/alacritty.toml";
        let text = fit_focus(
            parts_for(
                &node(
                    ".config/alacritty/alacritty.toml",
                    Some(dest),
                    ActionMode::Symlink,
                    SymlinkStatus::Valid,
                    None,
                ),
                dest,
            ),
            66,
        )
        .plain();
        assert!(chars_len(&text) <= 66, "{text}");
        assert!(text.starts_with("LINK  "), "{text}");
        assert!(text.ends_with("  linked"), "{text}");
        assert!(text.contains('…'), "{text}");
        assert!(text.contains("alacritty.toml"), "{text}");
    }

    #[test]
    fn zero_width_is_empty() {
        let text = fit_focus(
            parts_for(
                &node(
                    ".zshrc",
                    Some("/tmp/.zshrc"),
                    ActionMode::Symlink,
                    SymlinkStatus::Valid,
                    None,
                ),
                "/tmp/.zshrc",
            ),
            0,
        )
        .plain();
        assert_eq!(text, "");
    }
}
