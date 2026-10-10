use crate::{
    AppWindow, CourseItem, CourseTab, CoursesUi, MaterialItem,
    Screen::{self},
    UiState, database, filesystem,
    state::top_bar::refresh_file_view,
    types::course::Course,
    ui::{self},
};
use rusqlite::params;
use slint::{ComponentHandle, Model, ModelRc, ToSharedString, VecModel};
use std::path::PathBuf;
use std::rc::Rc;
use std::{io, time::Duration};

#[derive(Clone, Default)]
pub struct CourseState {}

impl CourseState {
    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let courses = database::from_sql::<Course>(
        "SELECT * FROM courses ORDER BY course_order, course_title COLLATE NOCASE, section_title COLLATE NOCASE"
            .to_owned(),
        &[],
    )?;
        let items = courses
            .iter()
            .map(|course| CourseItem {
                id: (&course.course_id).into(),
                title: (&course.course_title).into(),
                section: (&course.section_title).into(),
                code: (&course.section_code).into(),
                period: (&course.period).clone().unwrap_or("".to_string()).into(),
                hidden: course.hidden,
                icon: Default::default(),
            })
            .collect::<Vec<_>>();
        let global = ui.global::<CoursesUi>();
        global.set_period_character_count(
            items
                .iter()
                .map(|item| item.period.chars().count() as i32)
                .max()
                .unwrap_or(0)
                .max(1),
        );
        global.set_courses(ModelRc::new(VecModel::from(items)));
        refresh_file_view(ui);
        for course in courses.into_iter() {
            Self::course_item_image(course.course_id, course.logo_img_src);
        }
        Ok(())
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<CoursesUi>();
        if global
            .get_tabs()
            .as_any()
            .downcast_ref::<VecModel<CourseTab>>()
            .is_none()
        {
            global.set_tabs(ModelRc::from(Rc::new(VecModel::<CourseTab>::default())));
        }
        let weak = ui.as_weak();
        global.on_select_tab(move |index| {
            let Some(ui) = weak.upgrade() else { return };
            Self::select_tab(&ui, index);
        });
        let weak = ui.as_weak();
        global.on_close_tab(move |index| {
            if let Some(ui) = weak.upgrade() {
                Self::close_tab(&ui, index);
            }
        });
        let weak = ui.as_weak();
        global.on_navigate_course(move |course| {
            let Some(ui) = weak.upgrade() else { return };
            match Self::ensure_course_tab(&ui, &course, None) {
                Ok(index) => Self::select_tab(&ui, index as i32),
                Err(err) => log::warn!("Opening course failed: {err}"),
            }
        });
        let weak = ui.as_weak();
        global.on_navigate_course_with_image(move |course, icon| {
            let Some(ui) = weak.upgrade() else { return };
            match Self::ensure_course_tab(&ui, &course, Some(icon)) {
                Ok(index) => Self::select_tab(&ui, index as i32),
                Err(err) => log::warn!("Opening course failed: {err}"),
            }
        });
        let weak = ui.as_weak();
        global.on_navigate_folder(move |course, folder| {
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
        let weak = ui.as_weak();
        global.on_navigate_assignment(move |course, assignment| {
            let Some(ui) = weak.upgrade() else { return };
            ui.global::<crate::AssignmentUi>()
                .invoke_open(course, assignment);
        });
        global.on_drag_data(|index| {
            slint::SharedString::from(format!("foam-course-index:{index}")).into()
        });
        global.on_drag_index(|data| {
            data.plain_text()
                .ok()
                .and_then(|text| {
                    text.strip_prefix("foam-course-index:")
                        .and_then(|index| index.parse().ok())
                })
                .unwrap_or(-1)
        });
        let weak = ui.as_weak();
        global.on_reorder(move |from, to| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<CoursesUi>();
            let model = global.get_courses();
            let mut items: Vec<CourseItem> = (0..model.row_count())
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
                "UPDATE courses SET course_order = ? WHERE course_id = ?".to_owned(),
                items
                    .iter()
                    .enumerate()
                    .map(|(order, item)| (order as i64, item.id.to_string()))
                    .collect(),
            )
            .inspect(|_| log::debug!("Reordered course {from} to {to}"))
            .inspect_err(|err| log::warn!("Saving course order failed: {err}"))
            .ok();
            global.set_courses(ModelRc::new(VecModel::from(items)));
        });
        global.on_toggle_hidden(move |id| {
            database::execute(
                "UPDATE courses SET hidden = NOT hidden WHERE course_id = ?".to_owned(),
                params![id.to_string()],
            )
            .inspect(|_| log::debug!("Toggled hidden state for course with id {}", id))
            .inspect_err(|err| log::warn!("Toggling course hidden state failed: {err}"))
            .ok();
            ui::sync_ui();
        });
        let weak = ui.as_weak();
        global.on_set_period(move |id, value| {
            let Some(ui) = weak.upgrade() else {
                return value;
            };
            let model = ui.global::<CoursesUi>().get_courses();
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
                    "UPDATE courses SET period = ? WHERE course_id = ?".to_owned(),
                    params![period, id.to_string()],
                ) {
                    Ok(_) => {
                        item.period = period.into();
                        model.set_row_data(index, item.clone());
                        ui.global::<CoursesUi>().set_period_character_count(
                            (0..model.row_count())
                                .filter_map(|index| model.row_data(index))
                                .map(|item| item.period.chars().count() as i32)
                                .max()
                                .unwrap_or(0),
                        );
                    }
                    Err(err) => log::warn!("Saving course period failed: {err}"),
                }
            }
            item.period
        });
    }

    pub fn course_title(ui: &AppWindow, id: &str) -> slint::SharedString {
        let courses = ui.global::<CoursesUi>().get_courses();
        (0..courses.row_count())
            .filter_map(|index| courses.row_data(index))
            .find(|course| course.id == id)
            .map(|course| course.title)
            .unwrap_or_else(|| {
                crate::types::course::course(id)
                    .ok()
                    .flatten()
                    .map(|course| course.course_title.into())
                    .unwrap_or_else(|| id.into())
            })
    }

    fn course_tab_image(course: String) {
        (|| -> io::Result<Option<slint::Image>> {
            let Some(url) = database::from_sql_map(
                "SELECT logo_img_src FROM courses WHERE course_id = ?".into(),
                params![course],
                |row| row.get::<_, String>(0).map_err(io::Error::other),
            )?
            .into_iter()
            .next()
            .filter(|url| !url.trim().is_empty()) else {
                return Ok(None);
            };
            let course_id = course.to_owned();
            filesystem::asset_as_slint_img(&url, "", move |ui, result| match result {
                Ok(img) => {
                    let tabs = ui.global::<CoursesUi>().get_tabs();
                    if let Some(index) = (0..tabs.row_count()).find(|&index| {
                        tabs.row_data(index)
                            .is_some_and(|tab| tab.course_id == course_id)
                    }) && let Some(mut tab) = tabs.row_data(index)
                    {
                        tab.image = img;
                        tabs.set_row_data(index, tab);
                    } else {
                        log::warn!("Failed to set row data: {}", tabs.row_count());
                    }
                }
                Err(err) => log::warn!("Failed to load course tab image: {err}"),
            });
            Ok(None)
        })()
        .inspect_err(|err| log::warn!("Failed to spawn task: {err}"))
        .ok();
    }

    fn course_item_image(course_id: String, icon_url: String) {
        (|| -> io::Result<Option<slint::Image>> {
            filesystem::asset_as_slint_img(&icon_url, "", move |ui, result| match result {
                Ok(img) => {
                    let tabs = ui.global::<CoursesUi>().get_courses();
                    if let Some(index) = (0..tabs.row_count())
                        .find(|&index| tabs.row_data(index).is_some_and(|tab| tab.id == course_id))
                        && let Some(mut tab) = tabs.row_data(index)
                    {
                        tab.icon = img;
                        tabs.set_row_data(index, tab);
                    } else {
                        log::warn!("Failed to set row data: {}", tabs.row_count());
                    }
                }
                Err(err) => log::warn!("Failed to load course item image: {err}"),
            });
            Ok(None)
        })()
        .inspect_err(|err| log::warn!("Failed to spawn task: {err}"))
        .ok();
    }

    pub fn ensure_course_tab(
        ui: &AppWindow,
        course: &str,
        icon: Option<slint::Image>,
    ) -> io::Result<usize> {
        let global = ui.global::<CoursesUi>();
        let tabs = global.get_tabs();
        if let Some(index) = (0..tabs.row_count()).find(|&index| {
            tabs.row_data(index)
                .is_some_and(|tab| tab.course_id == course)
        }) {
            return Ok(index);
        }
        let model = tabs
            .as_any()
            .downcast_ref::<VecModel<CourseTab>>()
            .ok_or_else(|| io::Error::other("Course tabs are unavailable"))?;

        model.push(CourseTab {
            course_id: course.into(),
            title: Self::course_title(ui, course),
            image: icon.unwrap_or_else(|| {
                Self::course_tab_image(course.to_string());
                Default::default()
            }),
            ..Default::default()
        });

        let index = model.row_count() - 1;
        global.set_active_tab(index as i32);
        global.set_course_id(course.into());
        global.set_folder_id("".into());
        refresh_file_view(ui);
        Ok(index)
    }

    pub fn show_assignment(ui: &AppWindow, course: &str, id: &str) -> io::Result<()> {
        let parent = database::from_sql_map(
        "SELECT parent_id FROM materials WHERE course_id = ? AND type = 'assignment' AND material_id = ?".into(),
        params![course, id],
        |row| row.get::<_, String>(0).map_err(io::Error::other),
    )?
    .into_iter()
    .next()
    .ok_or_else(|| io::Error::other("Assignment material no longer exists"))?;
        let root = Self::root_folder(course)?;
        let index = Self::ensure_course_tab(ui, course, None)?;
        let global = ui.global::<CoursesUi>();
        global.set_active_tab(index as i32);
        global.set_course_id(course.into());
        global.set_folder_id(if parent == root {
            "".into()
        } else {
            parent.into()
        });
        refresh_file_view(ui);
        let tabs = global.get_tabs();
        if let Some(mut tab) = tabs.row_data(index) {
            tab.assignment_id = id.into();
            tab.document_id = "".into();
            tabs.set_row_data(index, tab);
        }
        Ok(())
    }

    pub fn close_tab(ui: &AppWindow, index: i32) {
        let global = ui.global::<CoursesUi>();
        let tabs = global.get_tabs();
        let Some(model) = tabs.as_any().downcast_ref::<VecModel<CourseTab>>() else {
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
            Screen::Materials | Screen::Assignment | Screen::Document
        );
        ui.global::<UiState>().set_focused_sidebar_item(-1);
        model.remove(row);
        if model.row_count() == 0 {
            global.set_active_tab(-1);
            global.set_course_id("".into());
            global.set_folder_id("".into());
            if showing_tab {
                ui.global::<UiState>().set_screen(Screen::Courses);
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
        let global = ui.global::<CoursesUi>();
        let Some(tab) = usize::try_from(index)
            .ok()
            .and_then(|index| global.get_tabs().row_data(index))
        else {
            return;
        };
        global.set_active_tab(index);
        global.set_course_id(tab.course_id.clone());
        global.set_folder_id(tab.folder_id);
        global.set_folder_title(tab.folder_title);
        global.set_parent_title(tab.parent_title);
        global.set_browser_error(tab.browser_error);
        global.set_materials(tab.materials);
        global.set_parent_materials(tab.parent_materials);
        if !tab.document_id.is_empty() {
            ui.global::<crate::DocumentUi>()
                .invoke_open(tab.course_id, tab.document_id);
        } else if tab.assignment_id.is_empty() {
            ui.global::<UiState>().set_screen(Screen::Materials);
        } else {
            ui.global::<crate::AssignmentUi>()
                .invoke_open(tab.course_id, tab.assignment_id);
        }
    }

    pub fn course_folders(ui: &AppWindow) -> Vec<MaterialItem> {
        let model = ui.global::<CoursesUi>().get_courses();
        (0..model.row_count())
            .filter_map(|index| model.row_data(index))
            .filter(|course| !course.hidden)
            .map(|course| MaterialItem {
                course_id: course.id,
                id: "".into(),
                title: course.title,
                kind: "course".into(),
            })
            .collect()
    }

    pub fn root_folder(course: &str) -> io::Result<String> {
        Ok(database::from_sql_map(
        "SELECT material_id FROM materials WHERE course_id = ? AND type = 'folder' AND parent_id = ''".into(),
        params![course],
        |row| row.get::<_, String>(0).map_err(io::Error::other),
    )?.into_iter().next().unwrap_or_else(|| "0".into()))
    }

    pub fn folder_metadata(course: &str, folder: &str) -> io::Result<(String, String)> {
        database::from_sql_map(
        "SELECT title, parent_id FROM materials WHERE course_id = ? AND material_id = ? AND type = 'folder'".into(),
        params![course, folder],
        |row| Ok((row.get(0).map_err(io::Error::other)?, row.get(1).map_err(io::Error::other)?)),
    )?.into_iter().next().ok_or_else(|| io::Error::other("Folder no longer exists: {folder}"))
    }

    pub fn children(course: &str, folder: &str) -> io::Result<Vec<MaterialItem>> {
        database::from_sql_map(
        "SELECT material_id, title, type FROM materials WHERE course_id = ? AND parent_id = ? ORDER BY type != 'folder', title COLLATE NOCASE, material_id".into(),
        params![course, folder],
        |row| Ok(MaterialItem {
            course_id: course.into(),
            id: row.get::<_, String>(0).map_err(io::Error::other)?.into(),
            title: row.get::<_, String>(1).map_err(io::Error::other)?.into(),
            kind: row.get::<_, String>(2).map_err(io::Error::other)?.into(),
        }),
    )
    }
}
