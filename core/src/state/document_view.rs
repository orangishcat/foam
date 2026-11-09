use std::io;

use crate::wry::{self, Source, ready};
use rusqlite::params;
use slint::{ComponentHandle, Model};

use super::task_view::task_menu_items;
use crate::ScreenMenuItem;
use crate::{
    AppWindow, FoldersUi, DocumentUi, Screen, UiState, database, filesystem,
    state::{folders::FolderState, state::State, top_bar::refresh_file_view},
    types::resource::Resource,
};
use anyhow;

#[derive(Clone, Default)]
pub struct DocumentViewerState {}

impl State for DocumentViewerState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        wry::init(ui);
        Ok(())
    }

    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        task_menu_items(ui)
    }
}

impl DocumentViewerState {
    pub fn open(ui: &AppWindow, folder: &str, id: &str) -> anyhow::Result<()> {
        let (parent, resource, canon_folder_id) = database::from_sql_map(
            "SELECT m.parent_id, m.data, m.folder_id
         FROM resources m WHERE m.type = 'document' AND m.resource_id = ?2
         AND (m.folder_id = ?1 OR EXISTS (
             SELECT 1 FROM json_each((SELECT data FROM folders WHERE folder_id = m.folder_id), '$.aliases') WHERE value = ?1
         ))"
            .into(),
            params![folder, id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("Document no longer exists"))?;
        let Resource::Document(document) = serde_json::from_str(&resource)? else {
            return Err(io::Error::other("Invalid document data").into());
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
            return Err(io::Error::other("Document has no resource URL").into());
        }
        let folder_id = canon_folder_id.as_str();
        let index = FolderState::ensure_folder_tab(ui, folder_id, None)?;
        let folders = ui.global::<FoldersUi>();
        folders.set_active_tab(index as i32);
        folders.set_folder_id(folder_id.into());
        folders.set_resource_folder_id(if parent == FolderState::root_folder(folder_id)? {
            "".into()
        } else {
            parent.into()
        });
        let tabs = folders.get_tabs();
        if let Some(mut tab) = tabs.row_data(index) {
            tab.document_id = id.into();
            tab.task_id = "".into();
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
        ui.global::<DocumentUi>().on_open(move |folder, id| {
            if let Some(ui) = weak.upgrade()
                && let Err(error) = Self::open(&ui, &folder, &id)
            {
                log::warn!("Opening document failed: {error}");
                wry::reset();
                ui.global::<DocumentUi>()
                    .set_error(error.to_string().into());
            }
        });
    }
}
