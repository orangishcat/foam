use crate::account::RequestError;
use std::{cell::RefCell, rc::Rc, time::Duration};

use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use wry::{
    Rect, WebView, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};

use crate::{
    AppWindow, AssignmentFile, AssignmentUi, AttachmentMenu, filesystem,
    wry::{Source, file_url},
};

struct Preview {
    window: AppWindow,
    files: Vec<AssignmentFile>,
    index: usize,
    generation: u64,
    webview: Option<WebView>,
    local_path: Option<std::path::PathBuf>,
}

fn wry_supported(extension: &str) -> bool {
    matches!(
        extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str(),
        "pdf"
            | "html"
            | "htm"
            | "txt"
            | "text"
            | "csv"
            | "json"
            | "xml"
            | "svg"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "ico"
            | "avif"
            | "mp3"
            | "mp4"
            | "m4a"
            | "wav"
            | "ogg"
            | "webm"
            | "mov"
    )
}

fn is_office_document(extension: &str) -> bool {
    matches!(
        extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str(),
        "doc"
            | "docx"
            | "docm"
            | "dot"
            | "dotx"
            | "dotm"
            | "xls"
            | "xlsx"
            | "xlsm"
            | "xlsb"
            | "xlt"
            | "xltx"
            | "xltm"
            | "ppt"
            | "pptx"
            | "pptm"
            | "pps"
            | "ppsx"
            | "ppsm"
            | "pot"
            | "potx"
            | "potm"
            | "odt"
            | "ott"
            | "ods"
            | "ots"
            | "odp"
            | "otp"
            | "odg"
            | "otg"
            | "odf"
            | "rtf"
    )
}

fn open_local(reveal: bool) {
    let preview = PREVIEW.with(|slot| slot.borrow().clone());
    let Some(preview) = preview else { return };
    let preview = preview.borrow();
    let Some(path) = &preview.local_path else {
        return;
    };
    #[cfg(target_os = "macos")]
    let result = {
        let mut command = std::process::Command::new("open");
        if reveal {
            command.arg("-R");
        }
        command.arg(path).spawn()
    };
    #[cfg(target_os = "windows")]
    let result = if reveal {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
    } else {
        std::process::Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(path)
            .spawn()
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open")
        .arg(if reveal {
            path.parent().unwrap_or(path)
        } else {
            path.as_path()
        })
        .spawn();
    if let Err(error) = result {
        preview
            .window
            .global::<AttachmentMenu>()
            .set_error(format!("Could not open attachment: {error}").into());
    }
}

thread_local! {
    static PREVIEW: RefCell<Option<Rc<RefCell<Preview>>>> = const { RefCell::new(None) };
}
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn open_browser(url: &str) {
    if !matches!(reqwest::Url::parse(url), Ok(url) if matches!(url.scheme(), "https" | "http")) {
        log::warn!("Cannot open invalid resource URL");
        return;
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(error) = result {
        log::warn!("Opening browser failed: {error}");
    }
}

fn bounds(window: &AppWindow) -> Rect {
    Rect {
        position: LogicalPosition::new(
            window.get_attachment_x() as f64,
            window.get_attachment_y() as f64,
        )
        .into(),
        size: LogicalSize::new(
            window.get_attachment_width().max(1.0) as f64,
            window.get_attachment_height().max(1.0) as f64,
        )
        .into(),
    }
}

fn close() {
    PREVIEW.with(|slot| {
        if let Some(preview) = slot.borrow_mut().take() {
            let mut preview = preview.borrow_mut();
            preview.generation += 1;
            preview.webview = None;
            preview.window.global::<AttachmentMenu>().set_visible(false);
        }
    });
}

fn action(action: &str) {
    if action == "close" {
        close();
        return;
    }
    let preview = PREVIEW.with(|slot| slot.borrow().clone());
    let Some(preview) = preview else { return };
    if action == "open" {
        let preview = preview.borrow();
        open_browser(&preview.files[preview.index].url);
    } else {
        let index = {
            let preview = preview.borrow();
            let delta = if action == "previous" { -1 } else { 1 };
            (preview.index as i32 + delta).clamp(0, preview.files.len() as i32 - 1) as usize
        };
        let changed = index != preview.borrow().index;
        if changed {
            load(&preview, index);
        }
    }
}

fn ready(preview: &Rc<RefCell<Preview>>, generation: u64, source: Source) {
    let mut preview = preview.borrow_mut();
    if generation != preview.generation {
        return;
    }
    if let Source::Local(path) = &source {
        match file_url(path).and_then(|url| {
            url.to_file_path()
                .map_err(|_| std::io::Error::other("Invalid attachment path"))
        }) {
            Ok(path) => {
                preview.local_path = Some(path);
                preview
                    .window
                    .global::<AttachmentMenu>()
                    .set_file_ready(true);
            }
            Err(error) => {
                preview
                    .window
                    .global::<AttachmentMenu>()
                    .set_error(error.to_string().into());
                return;
            }
        }
    }
    if preview.window.global::<AttachmentMenu>().get_unsupported() {
        if preview.local_path.is_none() {
            preview
                .window
                .global::<AttachmentMenu>()
                .set_error("Could not download attachment for external opening.".into());
        }
        return;
    }
    let result = (|| -> Result<WebView, Box<dyn std::error::Error + Send + Sync>> {
        let builder = WebViewBuilder::new().with_bounds(bounds(&preview.window));
        let builder = match source {
            Source::Local(path) => builder.with_url(file_url(&path)?.as_str()),
            Source::Remote(url) => {
                let headers = crate::account::active_account()?.resource_headers(&url)?;
                builder.with_url(url).with_headers(headers)
            }
        };
        preview
            .window
            .window()
            .with_winit_window(|window| builder.build_as_child(window))
            .ok_or("Preview window is unavailable")?
            .map_err(Into::into)
    })();
    match result {
        Ok(webview) => {
            preview
                .window
                .global::<AttachmentMenu>()
                .set_error("".into());
            preview.webview = Some(webview);
        }
        Err(error) => preview
            .window
            .global::<AttachmentMenu>()
            .set_error(format!("Could not preview item: {error}").into()),
    }
}

fn load(preview: &Rc<RefCell<Preview>>, index: usize) {
    let (file, generation) = {
        let mut preview = preview.borrow_mut();
        preview.index = index;
        preview.generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        preview.webview = None;
        preview.local_path = None;
        let file = preview.files[index].clone();
        let menu = preview.window.global::<AttachmentMenu>();
        menu.set_unsupported(!wry_supported(&file.extension));
        menu.set_extension(
            file.extension
                .trim_start_matches('.')
                .to_ascii_lowercase()
                .into(),
        );
        menu.set_is_office_document(is_office_document(&file.extension));
        menu.set_file_ready(false);
        preview
            .window
            .global::<AttachmentMenu>()
            .set_index(index as i32);
        preview
            .window
            .global::<AttachmentMenu>()
            .set_caption(file.title.clone());
        preview
            .window
            .global::<AttachmentMenu>()
            .set_error("".into());
        (file, preview.generation)
    };
    let url = file.url.to_string();
    let request = match crate::account::active_account()
        .map_err(|error| -> RequestError { error.into() })
        .and_then(|account| account.get_resource(&url))
    {
        Ok(request) => request,
        Err(error) => {
            preview
                .borrow()
                .window
                .global::<AttachmentMenu>()
                .set_error(error.to_string().into());
            return;
        }
    };
    let weak_window = preview.borrow().window.as_weak();
    let remote = url.clone();
    let extension = file.extension.to_string();
    let cached = filesystem::asset_from_url_result(request, &url, &extension, move |result| {
        let source = match result {
            Ok(path) => Source::Local(path),
            Err(_) => Source::Remote(remote),
        };
        let _ = weak_window.upgrade_in_event_loop(move |_window| {
            PREVIEW.with(|slot| {
                if let Some(preview) = slot.borrow().clone() {
                    ready(&preview, generation, source);
                }
            });
        });
    });
    if let Some(path) = cached {
        ready(preview, generation, Source::Local(path));
    }
}

pub fn open(ui: &AppWindow, index: i32) {
    let files = ui
        .global::<AssignmentUi>()
        .get_assignment()
        .attachments
        .iter()
        .collect::<Vec<_>>();
    open_items(ui, files, index, "attachment");
}

pub fn open_submissions(ui: &AppWindow, revision: i32) {
    let assignment = ui.global::<AssignmentUi>().get_assignment();
    let mut items = Vec::new();
    let mut selected = None;
    for (index, submission) in assignment.submissions.iter().enumerate() {
        if index as i32 == revision {
            selected = Some(items.len() as i32);
        }
        items.extend(submission.files.iter());
    }
    if let Some(index) = selected {
        open_items(ui, items, index, "submission file");
    }
}

fn open_items(ui: &AppWindow, files: Vec<AssignmentFile>, index: i32, kind: &str) {
    if index < 0 || index as usize >= files.len() {
        return;
    }
    close();
    let window = ui.clone_strong();
    let menu = window.global::<AttachmentMenu>();
    menu.set_count(files.len() as i32);
    menu.set_kind(kind.into());
    menu.on_step(|delta| action(if delta < 0 { "previous" } else { "next" }));
    menu.on_open_browser(|| action("open"));
    menu.on_open_external(|| open_local(false));
    menu.on_open_libreoffice_link(|| open_browser("https://www.libreoffice.org/"));
    menu.on_reveal_file(|| open_local(true));
    menu.on_close_preview(close);
    menu.set_visible(true);
    let preview = Rc::new(RefCell::new(Preview {
        window,
        files,
        index: index as usize,
        generation: 0,
        webview: None,
        local_path: None,
    }));
    let weak = Rc::downgrade(&preview);
    preview
        .borrow()
        .window
        .on_update_attachment_webview(move || {
            if let Some(preview) = weak.upgrade() {
                // Geometry changes can occur while a source is being installed.
                if let Ok(preview) = preview.try_borrow() {
                    if let Some(webview) = &preview.webview {
                        let _ = webview.set_bounds(bounds(&preview.window));
                    }
                }
            }
        });
    PREVIEW.with(|slot| *slot.borrow_mut() = Some(preview.clone()));
    // Let the overlay layout settle before placing the native child WebView.
    slint::Timer::single_shot(Duration::ZERO, move || load(&preview, index as usize));
}
