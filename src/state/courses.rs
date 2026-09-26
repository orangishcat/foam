use crate::{
    AppWindow, CourseItem, CoursesUi,
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
