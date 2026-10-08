use std::{ops::ControlFlow, rc::Rc, time::Duration};

use slint::{
    PlatformError,
    platform::{Clipboard, EventLoopProxy, Platform, WindowAdapter},
};

/// Share GTK's clipboard with the embedded WebKit views. Arboard's synchronous
/// X11 reads otherwise block the thread GTK needs to answer the selection request.
struct GtkClipboardBackend(i_slint_backend_winit::Backend);

pub fn select(event_loop: i_slint_backend_winit::EventLoopBuilder) -> Result<(), PlatformError> {
    let mut builder = i_slint_backend_winit::Backend::builder().with_event_loop_builder(event_loop);
    if let Ok(backend) = std::env::var("SLINT_BACKEND")
        && let Some(renderer) = backend.strip_prefix("winit-")
    {
        builder = builder.with_renderer_name(renderer);
    }
    slint::platform::set_platform(Box::new(GtkClipboardBackend(builder.build()?)))
        .map_err(|error| PlatformError::Other(error.to_string()))
}

fn clipboard(kind: Clipboard) -> Option<gtk::Clipboard> {
    let selection = match kind {
        Clipboard::DefaultClipboard => gtk::gdk::SELECTION_CLIPBOARD,
        Clipboard::SelectionClipboard => gtk::gdk::SELECTION_PRIMARY,
        _ => return None,
    };
    Some(gtk::Clipboard::get(&selection))
}

impl Platform for GtkClipboardBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        self.0.create_window_adapter()
    }

    fn bind_context(&self, context: i_slint_core::SlintContextWeak, token: i_slint_core::InternalToken) {
        self.0.bind_context(context, token);
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        self.0.run_event_loop()
    }

    fn process_events(
        &self,
        timeout: Option<Duration>,
        token: i_slint_core::InternalToken,
    ) -> Result<ControlFlow<()>, PlatformError> {
        self.0.process_events(timeout, token)
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        self.0.new_event_loop_proxy()
    }

    fn duration_since_start(&self) -> Duration {
        self.0.duration_since_start()
    }

    fn click_interval(&self) -> Duration {
        self.0.click_interval()
    }

    fn cursor_flash_cycle(&self) -> Duration {
        self.0.cursor_flash_cycle()
    }

    fn set_clipboard_text(&self, text: &str, kind: Clipboard) {
        if let Some(clipboard) = clipboard(kind) {
            clipboard.set_text(text);
        }
    }

    fn clipboard_text(&self, kind: Clipboard) -> Option<String> {
        // GTK services its event loop while waiting, including requests whose
        // owner is one of our WebKit views. All Slint paste paths use this hook.
        clipboard(kind)?.wait_for_text().map(|text| text.to_string())
    }

    fn debug_log(&self, arguments: std::fmt::Arguments) {
        self.0.debug_log(arguments);
    }

    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        self.0.open_url(url)
    }
}
