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

/// Instrument a webview builder and, with the test page enabled, point it at
/// the probe content instead of its intended URL.
#[allow(dead_code)] // The test-page path is exercised only via FOAM_WEBVIEW_TEST_PAGE.
pub fn decorate(builder: WebViewBuilder<'_>, url: &str) -> WebViewBuilder<'_> {
    let builder = instrument(builder);
    if test_page() {
        builder.with_html(TEST_HTML)
    } else {
        builder.with_url(url)
    }
}

/// Navigate an existing webview, substituting the test page when enabled.
pub fn navigate(webview: &WebView, url: &str) -> Result<(), wry::Error> {
    if test_page() {
        webview.load_html(TEST_HTML)
    } else {
        webview.load_url(url)
    }
}
