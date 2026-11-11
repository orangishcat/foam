use std::io;

use chrono::Utc;
use rusqlite::params;
use slint::{ComponentHandle, Model};

use super::state::menu_item;
use crate::state::state::State;
use crate::state::top_bar::refresh_file_view;
use crate::{
    AppWindow, FoldersUi, ResourceUi, ResourcesMenu, Screen, ScreenMenuItem, UiState, database,
    types::{document::Document, link::Link, resource::Resource},
};
use anyhow;

impl State for ResourcesState {
    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        let folders_ui = ui.global::<FoldersUi>();
        let active = folders_ui.get_active_tab();
        let tabs = folders_ui.get_tabs();
        let has_tab = active >= 0 && (active as usize) < tabs.row_count();
        let selected = ui.global::<ResourcesMenu>().get_selected_index();
        let resources_len = if has_tab {
            tabs.row_data(active as usize)
                .map_or(0, |tab| tab.resources.row_count())
        } else {
            0
        };
        let has_selection = has_tab && selected >= 0 && (selected as usize) < resources_len;
        vec![
            menu_item("Close tab", &["Control", "W"], has_tab),
            menu_item("View item", &["V"], has_selection),
            menu_item("Open in browser", &["O"], has_selection),
            menu_item("Unselect item", &["Escape"], has_selection),
        ]
    }
}

#[derive(Clone, Default)]
pub struct ResourcesState {}

impl ResourcesState {
    pub fn init_ui(&self, ui: &AppWindow) {
        let weak = ui.as_weak();
        ui.global::<ResourceUi>().on_create_resource(move |kind, title, url, resource_folder| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<ResourceUi>();
            match create_resource(&ui, kind, &title, &url, &resource_folder) {
                Ok(()) => {
                    global.set_create_visible(false);
                    global.set_create_error("".into());
                    refresh_file_view(&ui);
                }
                Err(err) => {
                    log::warn!("Creating material failed: {err}");
                    global.set_create_error(format!("{err}").into());
                }
            }
        });
        ui.global::<ResourceUi>().on_open_browser(|item| {
            let result = (|| -> anyhow::Result<String> {
                let account = crate::account::active_account()?;
                if item.kind == "task" {
                    return Ok(account.task_url(&item.id));
                }
                if item.kind == "course" || item.kind == "folder" || item.kind == "subfolder" {
                    let mut url = reqwest::Url::parse(&account.task_url(""))?;
                    url.set_path(&format!("/course/{}/materials", item.folder_id));
                    url.query_pairs_mut().append_pair("f", &item.id);
                    return Ok(url.into());
                }
                let urls = database::from_sql_map(
                    "SELECT json_extract(data, '$.resource.url') FROM resources
                     WHERE folder_id = ?1 AND resource_id = ?2 AND type = ?3"
                        .into(),
                    params![
                        item.folder_id.as_str(),
                        item.id.as_str(),
                        item.kind.as_str()
                    ],
                    |row| Ok(row.get::<_, String>(0)?),
                )?;
                urls.into_iter()
                    .find(|url| !url.trim().is_empty())
                    .ok_or_else(|| io::Error::other("Resource has no browser URL").into())
            })();
            match result {
                Ok(url) => crate::state::attachment_view::open_browser(&url),
                Err(error) => log::warn!("Opening resource in browser failed: {error}"),
            }
        });
        let global = ui.global::<FoldersUi>();
        let weak = ui.as_weak();
        global.on_open_resource_folder(move |folder, resource_folder| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<FoldersUi>();
            global.set_folder_id(folder);
            global.set_resource_folder_id(resource_folder);
            if let Ok(index) = usize::try_from(global.get_active_tab()) {
                let tabs = global.get_tabs();
                if let Some(mut tab) = tabs.row_data(index) {
                    tab.task_id = "".into();
                    tab.document_id = "".into();
                    tabs.set_row_data(index, tab);
                }
            }
            ui.global::<UiState>().set_screen(Screen::Resources);
            refresh_file_view(&ui);
        });
    }
}

/// Inserts a builtin document or link into `resources`, matching the
/// document/link resource JSON schema (tagged `{"type","resource"}` so the FTS
/// triggers and `document_view` read it the same way as synced rows).
fn create_resource(
    ui: &AppWindow,
    kind: i32,
    title: &str,
    url: &str,
    resource_folder: &str,
) -> anyhow::Result<()> {
    if title.trim().is_empty() || url.trim().is_empty() {
        anyhow::bail!("Title and URL are required");
    }
    let folder_id = ui.global::<FoldersUi>().get_folder_id().to_string();
    if folder_id.is_empty() {
        anyhow::bail!("Open a folder first");
    }
    let resource_id = format!("builtin-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
    let parent_id = if resource_folder.is_empty() {
        crate::state::folders::FolderState::root_folder(&folder_id)?
    } else {
        resource_folder.to_owned()
    };
    let data = match kind {
        1 => serde_json::to_string(&Resource::Link(Link {
            id: resource_id.clone(),
            title: title.to_owned(),
            url: url.to_owned(),
            ..Default::default()
        }))?,
        _ => serde_json::to_string(&Resource::Document(Document {
            id: resource_id.clone(),
            title: title.to_owned(),
            url: url.to_owned(),
            ..Default::default()
        }))?,
    };
    let kind = match kind {
        1 => "link",
        _ => "document",
    };
    let count = database::execute(
        "INSERT INTO resources (folder_id, resource_id, type, parent_id, source, title, data)
         VALUES (?1, ?2, ?3, ?4, 'builtin', ?5, ?6)
         ON CONFLICT(folder_id, type, resource_id) DO NOTHING"
            .to_owned(),
        params![folder_id, resource_id, kind, parent_id, title, data],
    )?;
    if count == 0 {
        anyhow::bail!("A resource with this id already exists");
    }
    log::info!("Created builtin {kind} resource {resource_id} in folder {folder_id}");
    Ok(())
}
