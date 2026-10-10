use std::io;

use crate::wry::{self, Source, ready};
use rusqlite::params;
use slint::{ComponentHandle, Model};

use crate::{
    AppWindow, CoursesUi, DocumentUi, Screen, UiState, database, filesystem,
    state::{courses::CourseState, top_bar::refresh_file_view},
    types::material::Material,
};

#[derive(Clone, Default)]
pub struct DocumentViewerState {}

impl DocumentViewerState {
    pub fn open(ui: &AppWindow, course: &str, id: &str) -> io::Result<()> {
        let (parent, material, canon_course_id) = database::from_sql_map(
            "SELECT m.parent_id, m.data, m.course_id
         FROM materials m LEFT JOIN courses c ON c.course_id = m.course_id
         WHERE m.type = 'document' AND m.material_id = ?2
         AND (m.course_id = ?1 OR EXISTS (
             SELECT 1 FROM json_each(c.aliases) WHERE value = ?1
         ))"
            .into(),
            params![course, id],
            |row| {
                Ok((
                    row.get::<_, String>(0).map_err(io::Error::other)?,
                    row.get::<_, String>(1).map_err(io::Error::other)?,
                    row.get::<_, String>(2).map_err(io::Error::other)?,
                ))
            },
        )?
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("Document no longer exists"))?;
        let Material::Document(document) =
            serde_json::from_str(&material).map_err(io::Error::other)?
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
        let course_id = canon_course_id.as_str();
        let index = CourseState::ensure_course_tab(ui, course_id, None)?;
        let courses = ui.global::<CoursesUi>();
        courses.set_active_tab(index as i32);
        courses.set_course_id(course_id.into());
        courses.set_folder_id(if parent == CourseState::root_folder(course_id)? {
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
        let weak = ui.as_weak();
        filesystem::asset_from_url(&url, &extension, move |result| {
            let source = match result {
                Ok(path) => Source::Local(path),
                Err(_) => Source::Remote(remote),
            };
            let _ = weak.upgrade_in_event_loop(move |ui| ready(&ui, generation, source));
        });
        Ok(())
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        wry::init(ui);
        let weak = ui.as_weak();
        ui.global::<DocumentUi>().on_open(move |course, id| {
            if let Some(ui) = weak.upgrade() {
                if let Err(error) = Self::open(&ui, &course, &id) {
                    log::warn!("Opening document failed: {error}");
                    wry::reset();
                    ui.global::<DocumentUi>()
                        .set_error(error.to_string().into());
                }
            }
        });
    }

    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        wry::init(ui);
        Ok(())
    }
}
