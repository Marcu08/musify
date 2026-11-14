//! Musify's Rune presentation layer.
//!
//! The UI is presentation only. Commands go to a `Backend`; events and read-only
//! library queries come back through the same trait. Rune never leaks into the
//! other crates.

mod backend;
mod i18n;
mod state;
mod theme;
mod view;

use std::sync::Arc;

use rune::{App, AppSpec};

pub use backend::Backend;

const WINDOW_SIZE: (f32, f32) = (1120.0, 680.0);

pub fn run(backend: Arc<dyn Backend>) {
    let backend_for_init = backend.clone();
    let backend_for_ready = backend.clone();
    let mut spec = AppSpec::new(
        i18n::APP_NAME,
        WINDOW_SIZE,
        || unreachable!("App::init supplies the state"),
        view::view,
    );
    spec.min_size = Some((980.0, 600.0));
    spec.scenes = state::scenes();
    spec.setup = Some(state::setup);
    spec.poll = Some(state::poll);
    spec.on_key = Some(state::on_key);
    let app = App::new(spec)
        .init(move || state::UiState::new(backend_for_init.clone()))
        // Windows may create the first window at 100% DPI before winit reports the
        // real scale. Resize once on the first ready frame so the viewport and the
        // native window agree at 125%/150% too.
        .on_ready(move |handle| {
            handle.resize_instant(WINDOW_SIZE);
            handle.set_position((0, 0));
            #[cfg(target_os = "windows")]
            if let Ok(Some(hwnd)) = handle.with_raw_handle(|raw| {
                use rune::raw_window_handle::RawWindowHandle;
                match raw.as_raw() {
                    RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                    _ => None,
                }
            }) {
                backend_for_ready.window_ready(hwnd);
            }
        });
    app.run();
}
