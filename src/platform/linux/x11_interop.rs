//! Narrow handling for GTK GLX errors observed with foreign X11 windows.
use gtk::prelude::*;
use slint::winit_030::winit;

pub(super) fn is_gtk_glx_destroy_window_error(
    gtk_display: usize,
    display: usize,
    opcode: i32,
    error_base: i32,
    event: &x11_dl::xlib::XErrorEvent,
) -> bool {
    // GLX protocol: GLXBadWindow = error base + 12, DestroyWindow = 32.
    display == gtk_display
        && i32::from(event.error_code) == error_base + 12
        && i32::from(event.request_code) == opcode
        && event.minor_code == 32
}

/// Register once on the GTK thread, before constructing winit's event loop.
/// The standalone probe can disable handling to observe the original failure.
pub fn install_x11_error_hook(workaround: bool) -> Result<(), Box<dyn std::error::Error>> {
    static INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if INSTALLED.get().is_some() {
        return Ok(());
    }
    let xlib = x11_dl::xlib::Xlib::open()?;
    let display = gtk::gdk::Display::default().ok_or("GTK display unavailable")?;
    let display = display
        .downcast::<gdkx11::X11Display>()
        .map_err(|_| "Embedded webviews require GTK's X11 display")?;
    // SAFETY: GTK owns this live X11 display; this runs on its UI thread.
    let gtk_display = unsafe { gdkx11::ffi::gdk_x11_display_get_xdisplay(display.as_ptr()) };
    let (mut opcode, mut event_base, mut error_base) = (0, 0, 0);
    // Query outside the error callback: Xlib calls there may reenter winit.
    let has_glx = unsafe {
        (xlib.XQueryExtension)(
            gtk_display.cast(),
            c"GLX".as_ptr(),
            &mut opcode,
            &mut event_base,
            &mut error_base,
        ) != 0
    };
    let gtk_display = gtk_display as usize;
    log::debug!(target: "foam::webview", "X11 integration: gtk_display={gtk_display:#x} glx={has_glx} opcode={opcode} error_base={error_base} workaround={workaround}");
    winit::platform::x11::register_xlib_error_hook(Box::new(move |display, event| {
        // SAFETY: winit passes an XErrorEvent valid for this callback's duration.
        let event = unsafe { &*event.cast::<x11_dl::xlib::XErrorEvent>() };
        // Unlike the upstream example, do not swallow every error numbered 170.
        let handled = workaround
            && has_glx
            && is_gtk_glx_destroy_window_error(
                gtk_display,
                display as usize,
                opcode,
                error_base,
                event,
            );
        if handled {
            log::debug!(target: "foam::webview", "Handled GTK GLXDestroyWindow error: resource={:#x} serial={}", event.resourceid, event.serial);
        } else {
            log::warn!(target: "foam::webview", "X11 error: display={display:p} resource={:#x} serial={} code={} request={} minor={}", event.resourceid, event.serial, event.error_code, event.request_code, event.minor_code);
        }
        handled
    }));
    let _ = INSTALLED.set(());
    Ok(())
}
