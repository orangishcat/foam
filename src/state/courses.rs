use crate::{
    AppWindow, CourseItem, CoursesUi, MaterialItem, Screen, UiState,
    api::schoology::{self, RequestResult},
    database,
    types::course::Course,
    ui::{self},
};
use rusqlite::params;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::io;

pub fn sync_ui(ui: &AppWindow) -> io::Result<()> {
    let courses = database::from_sql::<Course>(
        "SELECT * FROM courses ORDER BY course_order, course_title COLLATE NOCASE, section_title COLLATE NOCASE"
            .to_owned(),
        &[],
    )?;
    let items = courses
        .into_iter()
        .map(|course| CourseItem {
            id: course.course_id.into(),
            title: course.course_title.into(),
            section: course.section_title.into(),
            code: course.section_code.into(),
            period: course.period.unwrap_or("".to_string()).into(),
            hidden: course.hidden,
        })
        .collect::<Vec<_>>();
    let global = ui.global::<CoursesUi>();
    global.set_courses(ModelRc::new(VecModel::from(items)));
    refresh_browser(ui);
    let weak = ui.as_weak();
    global.on_open(move |course, folder, kind| {
        if kind != "course" && kind != "folder" {
            return;
        }
        let Some(ui) = weak.upgrade() else { return };
        let global = ui.global::<CoursesUi>();
        global.set_course_id(course);
        global.set_folder_id(folder);
        refresh_browser(&ui);
        ui.global::<UiState>().set_screen(Screen::Materials);
    });
    let weak = ui.as_weak();
    global.on_up(move || {
        let Some(ui) = weak.upgrade() else { return };
        let global = ui.global::<CoursesUi>();
        let course = global.get_course_id().to_string();
        let folder = global.get_folder_id().to_string();
        let result = if folder.is_empty() {
            global.set_course_id("".into());
            Ok(())
        } else {
            folder_metadata(&course, &folder).and_then(|(_, parent)| {
                global.set_folder_id(if parent == root_folder(&course)? {
                    "".into()
                } else {
                    parent.into()
                });
                Ok(())
            })
        };
        if let Err(err) = result {
            global.set_browser_error(format!("Unable to open parent: {err}").into());
        } else {
            refresh_browser(&ui);
        }
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

fn course_folders(ui: &AppWindow) -> Vec<MaterialItem> {
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

fn root_folder(course: &str) -> io::Result<String> {
    Ok(database::from_sql_map(
        "SELECT material_id FROM materials WHERE course_id = ? AND type = 'folder' AND parent_id = ''".into(),
        params![course],
        |row| row.get::<_, String>(0).map_err(io::Error::other),
    )?.into_iter().next().unwrap_or_else(|| "0".into()))
}

fn folder_metadata(course: &str, folder: &str) -> io::Result<(String, String)> {
    database::from_sql_map(
        "SELECT title, parent_id FROM materials WHERE course_id = ? AND material_id = ? AND type = 'folder'".into(),
        params![course, folder],
        |row| Ok((row.get(0).map_err(io::Error::other)?, row.get(1).map_err(io::Error::other)?)),
    )?.into_iter().next().ok_or_else(|| io::Error::other("Folder no longer exists"))
}

fn children(course: &str, folder: &str) -> io::Result<Vec<MaterialItem>> {
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

fn refresh_browser(ui: &AppWindow) {
    let global = ui.global::<CoursesUi>();
    global.set_browser_error("".into());
    let result = (|| -> io::Result<_> {
        let course = global.get_course_id().to_string();
        let folder = global.get_folder_id().to_string();
        if course.is_empty() {
            return Ok((
                "Courses".to_owned(),
                "Courses".to_owned(),
                course_folders(ui),
                course_folders(ui),
            ));
        }
        let course_title = crate::types::course::course(&course)?
            .ok_or_else(|| io::Error::other("Course no longer exists"))?
            .course_title;
        let root = root_folder(&course)?;
        if folder.is_empty() {
            return Ok((
                course_title,
                "Courses".to_owned(),
                children(&course, &root)?,
                course_folders(ui),
            ));
        }
        let (title, parent) = folder_metadata(&course, &folder)?;
        let parent_title = if parent == root {
            course_title
        } else {
            folder_metadata(&course, &parent)?.0
        };
        Ok((
            title,
            parent_title,
            children(&course, &folder)?,
            children(&course, &parent)?,
        ))
    })();
    match result {
        Ok((title, parent, items, siblings)) => {
            global.set_folder_title(title.into());
            global.set_parent_title(parent.into());
            global.set_materials(ModelRc::new(VecModel::from(items)));
            global.set_parent_materials(ModelRc::new(VecModel::from(siblings)));
        }
        Err(err) => {
            log::warn!("Loading materials failed: {err}");
            global.set_browser_error("Unable to load materials.".into());
            global.set_materials(ModelRc::default());
            global.set_parent_materials(ModelRc::default());
        }
    }
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
