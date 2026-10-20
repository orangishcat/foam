//! Platform-specific setup and behavior behind a small façade. Callers use
//! the functions here; the per-OS internals live in `linux`, `macos`, and
//! `windows` submodules.

use std::error::Error;

use slint::winit_030::winit;

use crate::{AppWindow, WindowChrome};
use slint::ComponentHandle;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

/// Select the Slint backend and prepare platform integrations before any
/// window exists.
pub fn init() -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "macos")]
    {
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .with_winit_window_attributes_hook(|attributes| {
                use winit::platform::macos::WindowAttributesExtMacOS;

                attributes
                    .with_titlebar_transparent(true)
                    .with_fullsize_content_view(true)
                    .with_title_hidden(true)
            })
            .select()?;
    }
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;

        // Wry child webviews require X11, including GTK's display connection.
        // Wayland desktops run this window through XWayland.
        gtk::gdk::set_allowed_backends("x11");
        gtk::init()?;
        linux::x11_interop::install_x11_error_hook(true)?;
        let mut event_loop_builder = winit::event_loop::EventLoop::with_user_event();
        event_loop_builder.with_x11();
        linux::select(event_loop_builder)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    Ok(())
}

/// Forward an edit-menu action (cut/copy/paste/select-all) to the focused
/// webview. No-op where the webview handles edit commands natively.
pub fn edit_webview(action: i32) {
    #[cfg(target_os = "macos")]
    macos::edit_webview(action);
    #[cfg(not(target_os = "macos"))]
    let _ = action;
}

/// Log webview/parent focus diagnostics when enabled. No-op elsewhere.
pub fn trace_parent_focus(window: &winit::window::Window, focused: bool) {
    #[cfg(target_os = "linux")]
    linux::webview_diagnostics::trace_parent_focus(window, focused);
    #[cfg(not(target_os = "linux"))]
    let _ = (window, focused);
}

/// Report whether the webview under a window owns the active state, falling
/// back to the winit-reported focus when unknown.
#[cfg(target_os = "linux")]
pub fn parent_active(window_id: winit::window::WindowId) -> Option<bool> {
    linux::webview_focus::parent_active(window_id)
}

/// Register a callback that fires when the activation state of a winit
/// window changes.
#[cfg(target_os = "linux")]
pub fn on_activation_change(callback: impl Fn(winit::window::WindowId, bool) + 'static) {
    linux::webview_focus::on_activation_change(callback);
}

/// Registration for per-webview focus and visibility lifecycle management,
/// used by [`crate::embedded_webview`].
#[cfg(target_os = "linux")]
pub(crate) use linux::webview_focus;

thread_local! {
    // Slint window used to mirror webview/window activation into the
    // custom-titlebar UI on linux and windows.
    static TITLEBAR_UI: std::cell::RefCell<Option<slint::Weak<AppWindow>>> =
        const { std::cell::RefCell::new(None) };
}

/// Apply platform window settings and register platform window handlers once
/// the app window exists.
pub fn setup_window(ui: &AppWindow) {
    ui.set_custom_titlebar(cfg!(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    )));
    ui.global::<WindowChrome>()
        .set_client_side(cfg!(any(target_os = "linux", target_os = "windows")));

    TITLEBAR_UI.with(|slot| *slot.borrow_mut() = Some(ui.as_weak()));

    #[cfg(target_os = "linux")]
    {
        use slint::winit_030::WinitWindowAccessor;

        let weak = ui.as_weak();
        on_activation_change(move |id, active| {
            if let Some(ui) = weak.upgrade() {
                ui.window().with_winit_window(|window| {
                    if window.id() == id {
                        ui.set_window_active(active);
                    }
                });
            }
        });

        let weak_ui = ui.as_weak();
        ui.global::<WindowChrome>().on_toggle_maximized(move || {
            if let Some(ui) = weak_ui.upgrade() {
                let window = ui.window();
                if !window.is_fullscreen() {
                    window.set_maximized(!window.is_maximized());
                }
            }
        });
    }
}

/// Platform-specific reactions to a winit window event. Runs before the
/// app's sidebar handling in [`crate::main`].
pub fn on_window_event(slint_window: &slint::Window, event: &winit::event::WindowEvent) {
    use slint::winit_030::WinitWindowAccessor;

    if let winit::event::WindowEvent::Focused(focused) = event {
        slint_window.with_winit_window(|window| trace_parent_focus(window, *focused));
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    if let winit::event::WindowEvent::Focused(focus) = event {
        TITLEBAR_UI.with(|ui| {
            let upgrade = ui.borrow().as_ref().and_then(|weak| weak.upgrade());
            if let Some(ui) = upgrade {
                #[cfg(target_os = "linux")]
                {
                    let active = slint_window
                        .with_winit_window(|window| {
                            parent_active(window.id()).unwrap_or(*focus)
                        })
                        .unwrap_or(*focus);
                    ui.set_window_active(active);
                }
                #[cfg(target_os = "windows")]
                ui.set_window_active(*focus);
            }
        });
    }

    #[cfg(target_os = "windows")]
    if let winit::event::WindowEvent::MouseInput {
        state: winit::event::ElementState::Pressed,
        ..
    } = event
    {
        // WebView2 children can retain keyboard focus after a click on Slint
        // content. Return it before Slint handles the text input.
        crate::api::schoology::onboarding::focus_input();
    }
}

/// Refreshes platform hooks when the app window gains focus.
pub fn on_focus_gained() {
    #[cfg(target_os = "macos")]
    macos::on_focus();
}

/// Start a secondary event pump when the platform requires driving its event
/// loop alongside Slint's; None otherwise.
pub fn start_event_pump() -> Option<slint::Timer> {
    #[cfg(target_os = "linux")]
    return Some(linux::start_gtk_pump());
    #[cfg(not(target_os = "linux"))]
    None
}

/// Webview instrumentation behind `FOAM_WEBVIEW_DIAGNOSTICS=1`.
#[cfg(target_os = "linux")]
pub use linux::webview_diagnostics;
