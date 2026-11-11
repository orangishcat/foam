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

/// Persists a document's inline HTML body as a cached `.html` file under the
/// attachments dir so the embedded viewer can load it with `Source::Local`.
fn write_body_html(id: &str, body: &str) -> anyhow::Result<std::path::PathBuf> {
    use sha2::Digest;
    let hash = format!("{:x}", sha2::Sha256::digest(format!("{id}\u{0}{body}")));
    let mut path = crate::config::config()
        .data_dir()
        .join("attachments")
        .join(&hash[..2]);
    std::fs::create_dir_all(&path)?;
    path.push(hash);
    path.set_extension("html");
    if !path.exists() {
        std::fs::write(&path, body)?;
    }
    Ok(path)
}

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
            .map(|file| {
                if file.preview_url.is_empty() {
                    file.download_path.clone()
                } else {
                    file.preview_url.clone()
                }
            })
            .unwrap_or(document.url.clone());
        // Page-style documents have no download URL; their content is inline
        // HTML in `body`, written as a local file for the viewer.
        let body_path = if url.trim().is_empty() {
            if document.body.trim().is_empty() {
                return Err(io::Error::other("Document has no resource URL").into());
            }
            Some(write_body_html(&document.id, &document.body)?)
        } else {
            None
        };
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
        if let Some(path) = body_path {
            ready(ui, generation, Source::Local(path));
            return Ok(());
        }
        if attachment.is_some_and(|file| {
            file.filesize > 0 && file.filesize as u64 > crate::config::config().max_attachment_bytes
        }) {
            ready(ui, generation, Source::Remote(url));
            return Ok(());
        }
        let remote = url.clone();
        let extension = attachment
            .map(|file| {
                if !file.preview_extension.is_empty() {
                    file.preview_extension.clone()
                } else if !file.extension.is_empty() {
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
