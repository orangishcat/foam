use crate::{
    AppWindow, CourseItem, CourseTab, CoursesUi, MaterialItem,
    Screen::{self},
    UiState,
    api::schoology::{self, RequestResult},
    database, filesystem,
    state::bottom_bar::refresh_file_view,
    types::course::Course,
    ui::{self},
};
use rusqlite::params;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

fn show_course_icon(id: String, path: PathBuf) {
    match filesystem::load_slint_img_thumbnail(&path, 84) {
        Ok(bytes) => ui::run_on_ui_thread(move |ui| {
            let model = ui.global::<CoursesUi>().get_courses();
            if let Some(index) = (0..model.row_count())
                .find(|&index| model.row_data(index).is_some_and(|item| item.id == id))
                && let Some(mut item) = model.row_data(index)
            {
                match slint::Image::load_from_data(&bytes, Some("png")) {
                    Ok(image) => {
                        item.icon = image;
                        model.set_row_data(index, item);
                    }
                    Err(error) => log::warn!("Loading prepared course icon failed: {error}"),
                }
            }
        }),
        Err(error) => log::warn!("Loading course icon {} failed: {error}", path.display()),
    }
}

pub fn sync_ui(ui: &AppWindow) -> io::Result<()> {
    let courses = database::from_sql::<Course>(
        "SELECT * FROM courses ORDER BY course_order, course_title COLLATE NOCASE, section_title COLLATE NOCASE"
            .to_owned(),
        &[],
    )?;
    let items = courses
        .into_iter()
        .map(|course| {
            let id = course.course_id.clone();
            let icon_url = course.logo_img_src.clone();
            if !icon_url.trim().is_empty() {
                std::thread::spawn(move || match schoology::internal_get_request(&icon_url) {
                    Ok(request) => {
                        let downloaded_id = id.clone();
                        if let Some(path) =
                            filesystem::asset_from_url(request, &icon_url, move |path| {
                                show_course_icon(downloaded_id, path)
                            })
                        {
                            show_course_icon(id, path);
                        }
                    }
                    Err(error) => log::warn!("Preparing course icon download failed: {error}"),
                });
            }
            CourseItem {
                id: course.course_id.into(),
                title: course.course_title.into(),
                section: course.section_title.into(),
                code: course.section_code.into(),
                period: course.period.unwrap_or("".to_string()).into(),
                hidden: course.hidden,
                icon: Default::default(),
            }
        })
        .collect::<Vec<_>>();
    let global = ui.global::<CoursesUi>();
    if global
        .get_tabs()
        .as_any()
        .downcast_ref::<VecModel<CourseTab>>()
        .is_none()
    {
        global.set_tabs(ModelRc::from(Rc::new(VecModel::<CourseTab>::default())));
    }
    global.set_courses(ModelRc::new(VecModel::from(items)));
    refresh_file_view(ui);
    let weak = ui.as_weak();
    global.on_select_tab(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        select_tab(&ui, index);
    });
    let weak = ui.as_weak();
    global.on_activate_course(move |course| {
        let Some(ui) = weak.upgrade() else { return };
        match ensure_course_tab(&ui, &course) {
            Ok(index) => select_tab(&ui, index as i32),
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
    global.on_hide(move |id| {
        database::execute(
            "UPDATE courses SET hidden = 1 WHERE course_id = ?".to_owned(),
            params![id.to_string()],
        )
        .inspect(|_| log::debug!("Hid course with id {}", id))
        .inspect_err(|err| log::warn!("Hiding course failed: {err}"))
        .ok();
        ui::sync_ui();
    });
    Ok(())
}

pub fn course_title(ui: &AppWindow, id: &str) -> slint::SharedString {
    let courses = ui.global::<CoursesUi>().get_courses();
    (0..courses.row_count())
        .filter_map(|index| courses.row_data(index))
        .find(|course| course.id == id)
        .map(|course| course.title)
        .unwrap_or_else(|| id.into())
}

pub fn ensure_course_tab(ui: &AppWindow, course: &str) -> io::Result<usize> {
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
        title: course_title(ui, course),
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
    let root = root_folder(course)?;
    let index = ensure_course_tab(ui, course)?;
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
        tabs.set_row_data(index, tab);
    }
    Ok(())
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
    if tab.assignment_id.is_empty() {
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

pub fn ensure_loaded() -> RequestResult<()> {
    let loaded = database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = 'courses_loaded'".to_owned(),
        &[],
        |row| row.get::<_, String>(0).map_err(std::io::Error::other),
    )?;
    if loaded.is_empty() {
        schoology::course::courses::scrape_courses()?;
        database::execute("INSERT INTO sync_state (key, data) VALUES ('courses_loaded', 'true') ON CONFLICT(key) DO NOTHING".to_owned(), &[])?;
    }
    Ok(())
}
