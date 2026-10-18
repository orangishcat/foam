//! Opt-in diagnostics shared by onboarding and the standalone focus reproducer.
use gtk::prelude::*;
use slint::winit_030::winit;
use wry::{WebView, WebViewBuilder, WebViewExtUnix};

pub fn enabled() -> bool {
    std::env::var("FOAM_WEBVIEW_DIAGNOSTICS").as_deref() == Ok("1")
}

#[allow(dead_code)] // Only onboarding uses this; the probe shares this module.
pub fn test_page() -> bool {
    enabled() && std::env::var("FOAM_WEBVIEW_TEST_PAGE").as_deref() == Ok("1")
}

pub fn trace_parent_focus(window: &winit::window::Window, focused: bool) {
    use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    if enabled() {
        log::info!(target: "foam::webview", "Slint/winit focused={focused} window={:?} display={:?}", window.window_handle().map(|h| h.as_raw()), window.display_handle().map(|h| h.as_raw()));
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum DomEvent {
    Ready,
    Focusin,
    Focusout,
    Focus,
    Blur,
    Pointerdown,
    Keydown,
    Input,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Element {
    Input,
    Textarea,
    Body,
    Other,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FocusMessage {
    event: DomEvent,
    target: Element,
    active: Element,
    document_focused: bool,
    top_frame: bool,
}

/// Log only fixed event categories and booleans, never page-controlled text,
/// keystrokes, element values, URLs, cookies, or credentials.
pub fn instrument(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    if !enabled() {
        return builder;
    }
    builder.with_initialization_script(include_str!("webview_focus.js"))
        .with_ipc_handler(|request| {
            if request.body().len() > 512 {
                return;
            }
            if let Ok(message) = serde_json::from_str::<FocusMessage>(request.body()) {
                log::info!(target: "foam::webview", "DOM event={:?} target={:?} active={:?} document_focused={} top_frame={}", message.event, message.target, message.active, message.document_focused, message.top_frame);
            }
        })
}

pub fn attach(webview: &WebView) {
    if !enabled() {
        return;
    }
    let widget = webview.webview();
    let xid = widget
        .window()
        .and_then(|w| w.downcast::<gdkx11::X11Window>().ok())
        .map(|w| w.xid());
    log::info!(target: "foam::webview", "GTK webview attached: xid={xid:?} has_focus={} is_focus={}", widget.has_focus(), widget.is_focus());
    widget.connect_focus_in_event(|widget, _| {
        log::info!(target: "foam::webview", "GTK focus-in has_focus={} is_focus={}", widget.has_focus(), widget.is_focus());
        gtk::glib::Propagation::Proceed
    });
    widget.connect_focus_out_event(|widget, _| {
        log::info!(target: "foam::webview", "GTK focus-out has_focus={} is_focus={}", widget.has_focus(), widget.is_focus());
        gtk::glib::Propagation::Proceed
    });
    widget.connect_button_press_event(|widget, _| {
        log::info!(target: "foam::webview", "GTK button-press has_focus={} is_focus={}", widget.has_focus(), widget.is_focus());
        gtk::glib::Propagation::Proceed
    });
}

pub const TEST_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Foam webview focus probe</title>
<style>body { font: 20px sans-serif; padding: 24px; } input, textarea { display: block; font: inherit; padding: 12px; margin: 16px 0; width: 260px; } :focus { outline: 3px solid #268bd2; }</style></head>
<body><h1>Webview focus probe</h1><label>Email<input type="email" placeholder="Email"></label>
<label>Password<input type="password" placeholder="Password"></label>
<button type="button">Tab target</button><label>Composition<textarea rows="2"></textarea></label></body></html>"#;

#[cfg(test)]
mod tests {
    use super::super::x11_interop::is_gtk_glx_destroy_window_error;
    use super::*;

    #[test]
    fn glx_filter_uses_server_codes_and_preserves_unrelated_errors() {
        let mut event = x11_dl::xlib::XErrorEvent {
            type_: 0,
            display: std::ptr::null_mut(),
            resourceid: 0,
            serial: 0,
            error_code: 170,
            request_code: 150,
            minor_code: 32,
        };
        assert!(is_gtk_glx_destroy_window_error(1, 1, 150, 158, &event));
        assert!(!is_gtk_glx_destroy_window_error(1, 2, 150, 158, &event));
        // The same numbers can identify another extension on a different server.
        assert!(!is_gtk_glx_destroy_window_error(1, 1, 151, 159, &event));
        event.error_code = 171;
        event.request_code = 151;
        assert!(is_gtk_glx_destroy_window_error(1, 1, 151, 159, &event));
        event.minor_code = 31;
        assert!(!is_gtk_glx_destroy_window_error(1, 1, 151, 159, &event));
        event.minor_code = 32;
        event.error_code = 3; // Core BadWindow must never be suppressed.
        assert!(!is_gtk_glx_destroy_window_error(1, 1, 151, 159, &event));
    }

    #[test]
    fn dom_diagnostics_reject_page_text_and_input_contents() {
        let valid = r#"{"event":"input","target":"input","active":"input","document_focused":false,"top_frame":true}"#;
        assert!(serde_json::from_str::<FocusMessage>(valid).is_ok());
        let with_value = valid.replace(
            "\"top_frame\":true",
            "\"top_frame\":true,\"value\":\"secret\"",
        );
        assert!(serde_json::from_str::<FocusMessage>(&with_value).is_err());
        assert!(
            serde_json::from_str::<FocusMessage>(
                &valid.replace("\"event\":\"input\"", "\"event\":\"secret\"")
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<FocusMessage>(
                &valid.replace("\"active\":\"input\"", "\"active\":\"secret\"")
            )
            .is_err()
        );
    }
}
