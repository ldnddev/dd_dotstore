use crate::domain::AppState;
use ratatui::Frame;
use ratatui::layout::Rect;
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
    let colors = &state.theme.colors;
    let kind = match toast.level {
        ToastLevel::Info => super::toast_chrome::ToastKind::Info,
        ToastLevel::Success => super::toast_chrome::ToastKind::Success,
        ToastLevel::Warning => super::toast_chrome::ToastKind::Warning,
        ToastLevel::Error => super::toast_chrome::ToastKind::Error,
    };
    let areas = super::toast_chrome::render_toast_stack(
        f,
        area,
        &[super::toast_chrome::ToastLine {
            kind,
            message: toast.message.as_str(),
        }],
        super::toast_chrome::ToastPaint {
            modal_background: colors.modal_background,
            text: colors.text_primary,
            success: colors.success,
            info: colors.info,
            warning: colors.warning,
            error: colors.error,
        },
        0,
    );
    state.pointer.toast_area = areas.into_iter().next();
}
