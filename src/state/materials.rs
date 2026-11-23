use std::io;

use slint::{ComponentHandle, Model};

use super::state::menu_item;
use crate::state::state::State;
use crate::state::top_bar::refresh_file_view;
use crate::{AppWindow, CoursesUi, MaterialUi, MaterialsMenu, Screen, ScreenMenuItem, UiState};
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
            menu_item("Refresh folder", &["Control", "R"], has_tab),
        ]
    }
}

#[derive(Clone, Default)]
pub struct MaterialsState {}

/// Resolve the Schoology web URL for a material. Notifications and the course
/// materials list share this so every material type has a browser action.
pub(crate) fn browser_url(course_id: &str, kind: &str, id: &str) -> anyhow::Result<String> {
    let account = crate::account::active_account()?;
    let base = |path: &str| -> anyhow::Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&account.assignment_url(""))?;
        url.set_path(path);
        Ok(url)
    };
    match kind {
        "assignment" => Ok(account.assignment_url(id)),
        "assessment" => Ok(base(&format!("/assessment/{id}"))?.into()),
        "course" => Ok(base(&format!("/course/{course_id}/materials"))?.into()),
        "folder" => {
            let mut url = base(&format!("/course/{course_id}/materials"))?;
            url.query_pairs_mut().append_pair("f", id);
            Ok(url.into())
        }
        "document" | "link" => Ok(base(&format!("/course/{course_id}/materials/gp/{id}"))?.into()),
        _ => Err(io::Error::other(format!("Unsupported material type: {kind}")).into()),
    }
}

impl MaterialsState {
    pub fn init_ui(&self, ui: &AppWindow) {
        ui.global::<MaterialUi>().on_open_browser(|item| {
            match browser_url(&item.course_id, &item.kind, &item.id) {
                Ok(url) => crate::state::attachment_view::open_browser(&url),
                Err(error) => log::warn!("Opening material in browser failed: {error}"),
            }
        });
        let global = ui.global::<CoursesUi>();
        let weak = ui.as_weak();
        let refresh_weak = weak.clone();
        global.on_refresh_active_folder(move || {
            let Some(ui) = refresh_weak.upgrade() else {
                return;
            };
            crate::state::courses::CourseState::refresh_active_folder(&ui);
        });
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
            crate::state::courses::CourseState::refresh_folder_if_stale(&ui);
        });
    }
}
