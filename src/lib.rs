pub mod actions;
pub mod app;
pub mod domain;
pub mod input;
pub mod theme;
pub mod tree;
pub mod ui;

/// Compatibility shim: tests and older call sites use `inputs::handle_key`.
pub mod inputs {
    pub use crate::input::{handle_key, handle_mouse};
}

/// Compatibility shim for one release.
pub mod state {
    pub use crate::domain::*;
    pub use crate::theme::*;
}

pub mod toast {
    pub use crate::ui::toast::{TOAST_DURATION, Toast, ToastLevel};
}
