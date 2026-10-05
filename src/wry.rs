use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
};

use ::wry::{
    Rect, WebView, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

use crate::{AppWindow, DocumentUi, Screen, UiState};

pub enum Source {
    Local(PathBuf),
    Remote(String),
}

#[derive(Default)]
struct Viewer {
    generation: u64,
    pending: Option<Source>,
    webview: Option<WebView>,
}

thread_local! {
    static VIEWER: RefCell<Viewer> = RefCell::default();
}

/// Reset the viewer and invalidate outstanding downloads. Call on the UI thread.
pub fn reset() -> u64 {
    VIEWER.with(|viewer| {
        let mut viewer = viewer.borrow_mut();
        viewer.generation += 1;
        viewer.webview = None;
        viewer.pending = None;
        viewer.generation
    })
}

/// Queue a source only if it belongs to the current viewer. Call on the UI thread.
pub fn ready(ui: &AppWindow, generation: u64, source: Source) {
    VIEWER.with(|viewer| {
        let mut viewer = viewer.borrow_mut();
        if viewer.generation == generation {
            viewer.pending = Some(source);
        }
    });
    update(ui);
}

pub fn init(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_update_document_webview(move || {
        if let Some(ui) = weak.upgrade() {
            update(&ui);
        }
    });
}

pub(crate) fn file_url(path: &Path) -> io::Result<reqwest::Url> {
    let path = path.canonicalize()?;
    reqwest::Url::from_file_path(path).map_err(|_| io::Error::other("Invalid document file path"))
}

fn update(ui: &AppWindow) {
    let visible = ui.global::<UiState>().get_screen() == Screen::Document;
    let bounds = Rect {
        position: LogicalPosition::new(ui.get_document_x() as f64, ui.get_document_y() as f64)
            .into(),
        size: LogicalSize::new(
            ui.get_document_width().max(0.0) as f64,
            ui.get_document_height().max(0.0) as f64,
        )
        .into(),
    };
    VIEWER.with(|viewer| {
        let mut viewer = viewer.borrow_mut();
        if visible && let Some(source) = viewer.pending.take() {
            ui.window().with_winit_window(|window| {
                let builder = WebViewBuilder::new().with_bounds(bounds);
                let builder = match source {
                    Source::Remote(url) => match crate::api::schoology::resource_headers(&url) {
                        Ok(headers) => builder.with_url(url).with_headers(headers),
                        Err(error) => {
                            ui.global::<DocumentUi>().set_error(
                                format!("Could not authenticate document request: {error}").into(),
                            );
                            return;
                        }
                    },
                    Source::Local(path) => match file_url(&path) {
                        Ok(url) => builder.with_url(url.as_str()),
                        Err(error) => {
                            ui.global::<DocumentUi>().set_error(
                                format!("Could not open cached document: {error}").into(),
                            );
                            return;
                        }
                    },
                };
                match builder.build_as_child(window) {
                    Ok(webview) => viewer.webview = Some(webview),
                    Err(error) => ui
                        .global::<DocumentUi>()
                        .set_error(format!("Could not create document viewer: {error}").into()),
                }
            });
        }
        if let Some(webview) = &viewer.webview {
            if let Err(error) = webview
                .set_visible(visible)
                .and_then(|_| webview.set_bounds(bounds))
            {
                log::warn!("Updating document viewer failed: {error}");
            }
        }
    });
}
