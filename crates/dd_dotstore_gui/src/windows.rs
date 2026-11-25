//! App id and window options.
//!
//! On Wayland `app_id` is the xdg app id. On X11 it is the window class.
//! The desktop file uses the same string as `StartupWMClass`.

use gpui::{
    App, Bounds, Pixels, SharedString, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

pub const APP_ID: &str = "dev.ldnddev.dd_dotstore";
pub const WINDOW_TITLE: &str = "dd_dotstore";

pub fn main_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        app_id: Some(APP_ID.to_string()),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from(WINDOW_TITLE)),
            ..Default::default()
        }),
        focus: true,
        show: true,
        ..Default::default()
    }
}

pub fn centered_bounds(cx: &App) -> Bounds<Pixels> {
    Bounds::centered(None, size(px(1280.0), px(800.0)), cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn main_window_is_1280_by_800_with_the_app_id(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let bounds = centered_bounds(cx);
            let options = main_window_options(bounds);
            assert_eq!(options.app_id.as_deref(), Some(APP_ID));
            assert_eq!(APP_ID, "dev.ldnddev.dd_dotstore");
            let size = options.window_bounds.expect("bounds").get_bounds().size;
            assert_eq!(size, gpui::size(px(1280.0), px(800.0)));
            let title = options
                .titlebar
                .as_ref()
                .and_then(|bar| bar.title.as_ref())
                .map(SharedString::to_string);
            assert_eq!(title.as_deref(), Some(WINDOW_TITLE));
        });
    }
}
