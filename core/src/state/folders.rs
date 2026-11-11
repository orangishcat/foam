use crate::state::state::State;
use crate::{
    AppWindow, FolderItem, FolderTab, FoldersUi, ResourceItem,
    Screen::{self},
    ScreenMenuItem, UiState, database, filesystem,
    state::{state::menu_item, top_bar::refresh_file_view},
    types::folder_info,
    ui::{self},
};
use anyhow;
use rusqlite::params;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::io;

impl State for FolderState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        let folders = folder_info::folders()?;
        let items = folders
            .iter()
            .map(|folder| FolderItem {
                id: (&folder.folder_id).into(),
                title: (&folder.title).into(),
                section: "".into(),
                code: "".into(),
                period: folder.period.clone().unwrap_or("".to_string()).into(),
                hidden: folder.hidden,
                icon: Default::default(),
            })
            .collect::<Vec<_>>();
        let global = ui.global::<FoldersUi>();
        global.set_period_character_count(
            items
                .iter()
                .map(|item| item.period.chars().count() as i32)
                .max()
                .unwrap_or(0)
                .max(1),
        );
        global.set_folders(ModelRc::new(VecModel::from(items)));
        refresh_file_view(ui);
        for folder in folders.into_iter() {
            Self::folder_item_image(folder.folder_id, folder.custom_icon.clone());
        }
        Ok(())
    }
    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        let show_hidden = ui.global::<FoldersUi>().get_show_hidden();
        vec![menu_item(
            if show_hidden {
                "Hide hidden folders"
            } else {
                "Show hidden folders"
            },
            &["H"],
            true,
        )]
    }
}

#[derive(Clone, Default)]
pub struct FolderState {}

impl FolderState {
    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<FoldersUi>();
        global.on_drag_data(|index| {
            slint::SharedString::from(format!("foam-folder-index:{index}")).into()
        });
        global.on_drag_index(|data| {
            data.plain_text()
                .ok()
                .and_then(|text| {
                    text.strip_prefix("foam-folder-index:")
                        .and_then(|index| index.parse().ok())
                })
                .unwrap_or(-1)
        });
        let weak = ui.as_weak();
        global.on_reorder(move |from, to| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<FoldersUi>();
            let model = global.get_folders();
            let mut items: Vec<FolderItem> = (0..model.row_count())
                .filter_map(|index| model.row_data(index))
                .collect();
            if from < 0 || from as usize >= items.len() {
                return;
            }
            let target = to.clamp(0, items.len() as i32 - 1) as usize;
            if from as usize == target {
                return;
            }
            let item = items.remove(from as usize);
            items.insert(target, item);
            database::bulk_execute(
                "UPDATE folders SET data = json_set(data, '$.folder_order', ?) WHERE folder_id = ?".to_owned(),
                items
                    .iter()
                    .enumerate()
                    .map(|(order, item)| (order as i64, item.id.to_string()))
                    .collect(),
            )
            .inspect(|_| log::debug!("Reordered folder {from} to {to}"))
            .inspect_err(|err| log::warn!("Saving folder order failed: {err}"))
            .ok();
            global.set_folders(ModelRc::new(VecModel::from(items)));
        });
        global.on_toggle_hidden(move |id| {
            database::execute(
                "UPDATE folders SET data = json_set(
                    data, '$.hidden', NOT coalesce(json_extract(data, '$.hidden'), 0)
                ) WHERE folder_id = ?"
                    .to_owned(),
                params![id.to_string()],
            )
            .inspect(|_| log::debug!("Toggled hidden state for folder with id {}", id))
            .inspect_err(|err| log::warn!("Toggling folder hidden state failed: {err}"))
            .ok();
            ui::sync_ui();
        });
        let weak = ui.as_weak();
        global.on_set_period(move |id, value| {
            let Some(ui) = weak.upgrade() else {
                return value;
            };
            let model = ui.global::<FoldersUi>().get_folders();
            let Some(index) = (0..model.row_count())
                .find(|&index| model.row_data(index).is_some_and(|item| item.id == id))
            else {
                return value;
            };
            let Some(mut item) = model.row_data(index) else {
                return value;
            };
            let period: String = value.chars().take(15).collect();
            if period != item.period.as_str() {
                match database::execute(
                    "UPDATE folders SET data = json_set(data, '$.period', ?) WHERE folder_id = ?".to_owned(),
                    params![period, id.to_string()],
                ) {
                    Ok(_) => {
                        item.period = period.into();
                        model.set_row_data(index, item.clone());
                        ui.global::<FoldersUi>().set_period_character_count(
                            (0..model.row_count())
                                .filter_map(|index| model.row_data(index))
                                .map(|item| item.period.chars().count() as i32)
                                .max()
                                .unwrap_or(0),
                        );
                    }
                    Err(err) => log::warn!("Saving folder period failed: {err}"),
                }
            }
            item.period
        });
    }

    pub fn folder_title(ui: &AppWindow, id: &str) -> slint::SharedString {
        let folders = ui.global::<FoldersUi>().get_folders();
        (0..folders.row_count())
            .filter_map(|index| folders.row_data(index))
            .find(|folder| folder.id == id)
            .map(|folder| folder.title)
            .unwrap_or_else(|| {
                crate::types::folder_info::folder(id)
                    .ok()
                    .flatten()
                    .map(|folder| folder.title.into())
                    .unwrap_or_else(|| id.into())
            })
    }

    pub(crate) fn folder_tab_image(folder: String) {
        (|| -> anyhow::Result<Option<slint::Image>> {
            let Some(url) = database::from_sql_map(
                "SELECT custom_icon FROM folders WHERE folder_id = ?".into(),
                params![folder],
                |row| Ok(row.get::<_, Option<String>>(0)?),
            )?
            .into_iter()
            .flatten()
            .next()
            .filter(|url| !url.trim().is_empty()) else {
                return Ok(None);
            };
            let folder_id = folder.to_owned();
            filesystem::asset_as_slint_img(&url, "", move |ui, result| match result {
                Ok(img) => {
                    let tabs = ui.global::<FoldersUi>().get_tabs();
                    if let Some(index) = (0..tabs.row_count()).find(|&index| {
                        tabs.row_data(index)
                            .is_some_and(|tab| tab.folder_id == folder_id)
                    }) && let Some(mut tab) = tabs.row_data(index)
                    {
                        tab.image = img;
                        tabs.set_row_data(index, tab);
                    } else {
                        log::warn!("Failed to set row data: {}", tabs.row_count());
                    }
                }
                Err(err) => log::warn!("Failed to load folder tab image: {err}"),
            });
            Ok(None)
        })()
        .inspect_err(|err| log::warn!("Failed to spawn task: {err}"))
        .ok();
    }

    fn folder_item_image(folder_id: String, icon_url: String) {
        filesystem::asset_as_slint_img(&icon_url, "", move |ui, result| match result {
            Ok(img) => {
                let tabs = ui.global::<FoldersUi>().get_folders();
                if let Some(index) = (0..tabs.row_count())
                    .find(|&index| tabs.row_data(index).is_some_and(|tab| tab.id == folder_id))
                    && let Some(mut tab) = tabs.row_data(index)
                {
                    tab.icon = img;
                    tabs.set_row_data(index, tab);
                } else {
                    log::warn!("Failed to set row data: {}", tabs.row_count());
                }
            }
            Err(err) => log::warn!("Failed to load folder item image: {err}"),
        });
    }

    pub fn ensure_folder_tab(
        ui: &AppWindow,
        folder: &str,
        icon: Option<slint::Image>,
    ) -> anyhow::Result<usize> {
        let global = ui.global::<FoldersUi>();
        let tabs = global.get_tabs();
        if let Some(index) = (0..tabs.row_count()).find(|&index| {
            tabs.row_data(index)
                .is_some_and(|tab| tab.folder_id == folder)
        }) {
            return Ok(index);
        }
        let model = tabs
            .as_any()
            .downcast_ref::<VecModel<FolderTab>>()
            .ok_or_else(|| io::Error::other("Folder tabs are unavailable"))?;

        model.push(FolderTab {
            folder_id: folder.into(),
            title: Self::folder_title(ui, folder),
            image: icon.unwrap_or_else(|| {
                Self::folder_tab_image(folder.to_string());
                Default::default()
            }),
            ..Default::default()
        });

        let index = model.row_count() - 1;
        global.set_active_tab(index as i32);
        global.set_folder_id(folder.into());
        global.set_resource_folder_id("".into());
        refresh_file_view(ui);
        Ok(index)
    }

    pub fn show_task(ui: &AppWindow, folder: &str, id: &str) -> anyhow::Result<()> {
        let parent = database::from_sql_map(
        "SELECT parent_id FROM resources WHERE folder_id = ? AND type = 'task' AND resource_id = ?".into(),
        params![folder, id],
        |row| Ok(row.get::<_, String>(0)?),
    )?
    .into_iter()
    .next()
    .ok_or_else(|| io::Error::other("Task no longer exists"))?;
        let root = Self::root_folder(folder)?;
        let index = Self::ensure_folder_tab(ui, folder, None)?;
        let global = ui.global::<FoldersUi>();
        global.set_active_tab(index as i32);
        global.set_folder_id(folder.into());
        global.set_resource_folder_id(if parent == root {
            "".into()
        } else {
            parent.into()
        });
        refresh_file_view(ui);
        let tabs = global.get_tabs();
        if let Some(mut tab) = tabs.row_data(index) {
            tab.task_id = id.into();
            tab.document_id = "".into();
            tabs.set_row_data(index, tab);
        }

        Ok(())
    }

    pub fn close_tab(ui: &AppWindow, index: i32) {
        let global = ui.global::<FoldersUi>();
        let tabs = global.get_tabs();
        let Some(model) = tabs.as_any().downcast_ref::<VecModel<FolderTab>>() else {
            return;
        };
        let Ok(row) = usize::try_from(index) else {
            return;
        };
        if row >= model.row_count() {
            return;
        }
        let active = global.get_active_tab();
        let showing_tab = matches!(
            ui.global::<UiState>().get_screen(),
            Screen::Resources | Screen::Task | Screen::Document
        );
        ui.global::<UiState>().set_focused_sidebar_item(-1);
        model.remove(row);
        if model.row_count() == 0 {
            global.set_active_tab(-1);
            global.set_folder_id("".into());
            global.set_resource_folder_id("".into());
            if showing_tab {
                ui.global::<UiState>().set_screen(Screen::Folders);
            }
        } else if active == index {
            let next = row.min(model.row_count() - 1) as i32;
            if showing_tab {
                Self::select_tab(ui, next);
            } else {
                global.set_active_tab(next);
            }
        } else if active > index {
            global.set_active_tab(active - 1);
        }
    }

    pub fn select_tab(ui: &AppWindow, index: i32) {
        let global = ui.global::<FoldersUi>();
        let Some(tab) = usize::try_from(index)
            .ok()
            .and_then(|index| global.get_tabs().row_data(index))
        else {
            return;
        };
        global.set_active_tab(index);
        global.set_folder_id(tab.folder_id.clone());
        global.set_resource_folder_id(tab.resource_folder_id);
        global.set_resource_folder_title(tab.resource_folder_title);
        global.set_parent_title(tab.parent_title);
        global.set_browser_error(tab.browser_error);
        global.set_resources(tab.resources);
        global.set_parent_resources(tab.parent_resources);
        if !tab.document_id.is_empty() {
            ui.global::<crate::DocumentUi>()
                .invoke_open(tab.folder_id, tab.document_id);
        } else if tab.task_id.is_empty() {
            ui.global::<UiState>().set_screen(Screen::Resources);
        } else {
            ui.global::<crate::TaskUi>()
                .invoke_open(tab.folder_id, tab.task_id);
        }
    }

    pub fn folder_list_items(ui: &AppWindow) -> Vec<ResourceItem> {
        let model = ui.global::<FoldersUi>().get_folders();
        (0..model.row_count())
            .filter_map(|index| model.row_data(index))
            .filter(|folder| !folder.hidden)
            .map(|folder| ResourceItem {
                folder_id: folder.id,
                id: "".into(),
                title: folder.title,
                kind: "course".into(),
                icon_color: slint::Color::default(),
            })
            .collect()
    }

    pub fn root_folder(folder: &str) -> anyhow::Result<String> {
        Ok(database::from_sql_map(
        "SELECT resource_id FROM resources WHERE folder_id = ? AND type = 'folder' AND parent_id = ''".into(),
        params![folder],
        |row| Ok(row.get::<_, String>(0)?),
    )?.into_iter().next().unwrap_or_else(|| "0".into()))
    }

    pub fn folder_metadata(folder: &str, resource_folder: &str) -> anyhow::Result<(String, String)> {
        database::from_sql_map(
        "SELECT title, parent_id FROM resources WHERE folder_id = ? AND resource_id = ? AND type = 'folder'".into(),
        params![folder, resource_folder],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?.into_iter().next().ok_or_else(|| io::Error::other("Folder no longer exists: {resource_folder}").into())
    }

    pub fn children(
        _ui: &AppWindow,
        folder: &str,
        resource_folder: &str,
    ) -> anyhow::Result<Vec<ResourceItem>> {
        let items = database::from_sql_map(
        "SELECT resource_id, title, type FROM resources WHERE folder_id = ? AND parent_id = ? ORDER BY type != 'folder', title COLLATE NATURAL_ORDER, resource_id".into(),
        params![folder, resource_folder],
        |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let kind: String = row.get(2)?;
            Ok(ResourceItem {
                folder_id: folder.into(),
                id: id.into(),
                title: title.into(),
                kind: kind.into(),
                icon_color: slint::Color::default(),
            })
        },
    )?;
        Ok(items)
    }
}
