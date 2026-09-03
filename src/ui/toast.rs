use crate::domain::AppState;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::time::{Duration, Instant};

pub const TOAST_DURATION: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Toast {
    pub fn new(level: ToastLevel, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level,
            created_at: Instant::now(),
            duration: TOAST_DURATION,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }
}

pub(crate) fn draw_toast(f: &mut Frame, state: &mut AppState, area: Rect) {
    let Some(toast) = &state.toast else {
        state.pointer.toast_area = None;
        return;
    };
    if area.width < 8 || area.height < 5 {
        state.pointer.toast_area = None;
        return;
    }

    let max_width = area.width.saturating_sub(2).min(50);
    let longest_line = toast
        .message
        .lines()
        .map(|line| line.chars().count() as u16)
        .max()
        .unwrap_or(0);
    let width = longest_line.saturating_add(4).clamp(24, max_width);
    let line_count = toast.message.lines().count().max(1) as u16;
    let height = line_count
        .saturating_add(2)
        .clamp(3, area.height.saturating_sub(1).min(7));
    let x = area.x + area.width.saturating_sub(width).saturating_sub(1);
    let y = area.y + area.height.saturating_sub(height).saturating_sub(1);
    let toast_area = Rect::new(x, y, width, height);
    state.pointer.toast_area = Some(toast_area);

    let (title, border_style) = match toast.level {
        ToastLevel::Info => ("Info", state.theme.info),
        ToastLevel::Success => ("Success", state.theme.valid),
        ToastLevel::Warning => ("Warning", state.theme.warning),
        ToastLevel::Error => ("Error", state.theme.error),
    };

    let text = Paragraph::new(toast.message.as_str())
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(state.theme.modal),
        )
        .style(state.theme.modal_text)
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, toast_area);
    f.render_widget(text, toast_area);
}
