use std::io;

use crate::wry::{self, Source, ready};
use rusqlite::params;
use slint::{ComponentHandle, Model};

use crate::{
    AppWindow, CoursesUi, DocumentUi, Screen, UiState, database, filesystem,
    state::{
        bottom_bar::refresh_file_view,
        courses::{ensure_course_tab, root_folder},
    },
    types::material::Material,
};

fn open(ui: &AppWindow, course: &str, id: &str) -> io::Result<()> {
    let (parent, material) = database::from_sql_map(
        "SELECT parent_id, data FROM materials WHERE course_id = ? AND type = 'document' AND material_id = ?".into(),
        params![course, id],
        |row| Ok((row.get::<_, String>(0).map_err(io::Error::other)?,
            row.get::<_, String>(1).map_err(io::Error::other)?)),
    )?.into_iter().next().ok_or_else(|| io::Error::other("Document no longer exists"))?;
    let Material::Document(document) = serde_json::from_str(&material).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("Invalid document data"));
    };
    let attachment = document
        .attachments
        .files
        .file
        .iter()
        .find(|file| !file.download_path.is_empty());
    let url = attachment
        .map(|file| file.download_path.clone())
        .unwrap_or(document.url);
    if url.trim().is_empty() {
        return Err(io::Error::other("Document has no resource URL"));
    }
    let index = ensure_course_tab(ui, course, None)?;
    let courses = ui.global::<CoursesUi>();
    courses.set_active_tab(index as i32);
    courses.set_course_id(course.into());
    courses.set_folder_id(if parent == root_folder(course)? {
        "".into()
    } else {
        parent.into()
    });
    let tabs = courses.get_tabs();
    if let Some(mut tab) = tabs.row_data(index) {
        tab.document_id = id.into();
        tab.assignment_id = "".into();
        tabs.set_row_data(index, tab);
    }
    let global = ui.global::<DocumentUi>();
    global.set_id(id.into());
    global.set_title(document.title.into());
    global.set_error("".into());
    ui.global::<UiState>().set_screen(Screen::Document);
    refresh_file_view(ui);
    let generation = wry::reset();
    if attachment.is_some_and(|file| {
        file.filesize > 0 && file.filesize as u64 > crate::config::config().max_attachment_bytes
    }) {
        ready(ui, generation, Source::Remote(url));
        return Ok(());
    }
    let request = crate::api::schoology::resource_get_request(&url)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let remote = url.clone();
    let extension = attachment
        .map(|file| {
            if !file.extension.is_empty() {
                file.extension.clone()
            } else {
                std::path::Path::new(&file.filename)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or_default()
                    .to_owned()
            }
        })
        .unwrap_or_default();
    let downloaded_extension = extension.clone();
    let weak = ui.as_weak();
    let cached = filesystem::asset_from_url_result(request, &url, move |result| {
        let source = match result {
            Ok(path) => Source::Local(path, downloaded_extension),
            Err(_) => Source::Remote(remote),
        };
        let _ = weak.upgrade_in_event_loop(move |ui| ready(&ui, generation, source));
    });
    if let Some(path) = cached {
        ready(ui, generation, Source::Local(path, extension));
    }
    Ok(())
}

pub fn init(ui: &AppWindow) {
    wry::init(ui);
    let weak = ui.as_weak();
    ui.global::<DocumentUi>().on_open(move |course, id| {
        if let Some(ui) = weak.upgrade() {
            if let Err(error) = open(&ui, &course, &id) {
                log::warn!("Opening document failed: {error}");
                wry::reset();
                ui.global::<DocumentUi>()
                    .set_error(error.to_string().into());
            }
        }
    });
}
