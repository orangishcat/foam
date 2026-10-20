//! Wry child views with platform focus and visibility lifecycle management.
use slint::winit_030::winit::window::Window;
use std::{cell::Cell, ops::Deref};
use wry::{Rect, WebView, WebViewBuilder};

pub(crate) struct EmbeddedWebView {
    // Fields drop in declaration order: unregister before destroying WebKit.
    #[cfg(target_os = "linux")]
    focus: crate::platform::webview_focus::Registration,
    inner: WebView,
    visible: Cell<bool>,
}

impl EmbeddedWebView {
    pub fn build(
        builder: WebViewBuilder<'_>,
        parent: &Window,
        bounds: Rect,
        visible: bool,
    ) -> wry::Result<Self> {
        let builder = builder.with_bounds(bounds).with_visible(visible);
        #[cfg(target_os = "linux")]
        let builder = builder.with_focused(false);
        let inner = builder.build_as_child(parent)?;
        // Wry 0.57 records its X11 container after its initial hide call.
        #[cfg(target_os = "linux")]
        if !visible {
            inner.set_visible(false)?;
        }
        #[cfg(target_os = "linux")]
        let focus = crate::platform::webview_focus::Registration::new(
            &inner, parent, bounds, visible,
        )?;
        Ok(Self {
            #[cfg(target_os = "linux")]
            focus,
            inner,
            visible: Cell::new(visible),
        })
    }

    pub fn set_visible(&self, visible: bool) -> wry::Result<()> {
        if visible == self.visible.get() {
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        if !visible {
            self.focus.set_visible(false);
        }
        self.inner.set_visible(visible)?;
        self.visible.set(visible);
        #[cfg(target_os = "linux")]
        if visible {
            self.focus.set_visible(true);
        }
        Ok(())
    }

    pub fn set_bounds(&self, bounds: Rect) -> wry::Result<()> {
        self.inner.set_bounds(bounds)?;
        #[cfg(target_os = "linux")]
        self.focus.set_bounds(bounds);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn focus(&self) -> wry::Result<()> {
        #[cfg(target_os = "linux")]
        self.focus.focus();
        #[cfg(not(target_os = "linux"))]
        self.inner.focus()?;
        Ok(())
    }
}

impl Deref for EmbeddedWebView {
    type Target = WebView;
    fn deref(&self) -> &WebView {
        &self.inner
    }
}
