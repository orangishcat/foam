use std::io;

use rusqlite::params;
use slint::{ComponentHandle, Model};

use super::state::menu_item;
use crate::state::state::State;
use crate::state::top_bar::refresh_file_view;
use crate::{
    AppWindow, CoursesUi, MaterialUi, MaterialsMenu, Screen, ScreenMenuItem, UiState, database,
};
use anyhow;

impl State for MaterialsState {
    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        let courses_ui = ui.global::<CoursesUi>();
        let active = courses_ui.get_active_tab();
        let tabs = courses_ui.get_tabs();
        let has_tab = active >= 0 && (active as usize) < tabs.row_count();
        let selected = ui.global::<MaterialsMenu>().get_selected_index();
        let materials_len = if has_tab {
            tabs.row_data(active as usize)
                .map_or(0, |tab| tab.materials.row_count())
        } else {
            0
        };
        let has_selection = has_tab && selected >= 0 && (selected as usize) < materials_len;
        vec![
            menu_item("Close tab", &["Control", "W"], has_tab),
            menu_item("View item", &["V"], has_selection),
            menu_item("Open in browser", &["O"], has_selection),
            menu_item("Unselect item", &["Escape"], has_selection),
        ]
    }
}

#[derive(Clone, Default)]
pub struct MaterialsState {}

impl MaterialsState {
    pub fn init_ui(&self, ui: &AppWindow) {
        ui.global::<MaterialUi>().on_open_browser(|item| {
            let result = (|| -> anyhow::Result<String> {
                let account = crate::account::active_account()?;
                if item.kind == "assignment" {
                    return Ok(account.assignment_url(&item.id));
                }
                if item.kind == "folder" || item.kind == "course" {
                    let mut url = reqwest::Url::parse(&account.assignment_url(""))?;
                    url.set_path(&format!("/course/{}/materials", item.course_id));
                    if item.kind == "folder" {
                        url.query_pairs_mut().append_pair("f", &item.id);
                    }
                    return Ok(url.into());
                }
                let urls = database::from_sql_map(
                    "SELECT json_extract(data, '$.material.url') FROM materials
                     WHERE course_id = ?1 AND material_id = ?2 AND type = ?3"
                        .into(),
                    params![
                        item.course_id.as_str(),
                        item.id.as_str(),
                        item.kind.as_str()
                    ],
                    |row| Ok(row.get::<_, String>(0)?),
                )?;
                urls.into_iter()
                    .find(|url| !url.trim().is_empty())
                    .ok_or_else(|| io::Error::other("Material has no browser URL").into())
            })();
            match result {
                Ok(url) => crate::state::attachment_view::open_browser(&url),
                Err(error) => log::warn!("Opening material in browser failed: {error}"),
            }
        });
        let global = ui.global::<CoursesUi>();
        let weak = ui.as_weak();
        global.on_open_folder(move |course, folder| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<CoursesUi>();
            global.set_course_id(course);
            global.set_folder_id(folder);
            if let Ok(index) = usize::try_from(global.get_active_tab()) {
                let tabs = global.get_tabs();
                if let Some(mut tab) = tabs.row_data(index) {
                    tab.assignment_id = "".into();
                    tab.document_id = "".into();
                    tabs.set_row_data(index, tab);
                }
            }
            ui.global::<UiState>().set_screen(Screen::Materials);
            refresh_file_view(&ui);
        });
    }
}
