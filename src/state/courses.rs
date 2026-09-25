use crate::{
    AppWindow, CourseItem, CoursesUi,
    api::schoology::{self, RequestResult},
    database,
    types::course::Course,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::io;

const ORDER_KEY: &str = "course_order";

pub fn sync_ui(ui: &AppWindow) -> io::Result<()> {
    let courses = database::from_sql::<Course>(
        "SELECT * FROM courses ORDER BY course_title COLLATE NOCASE, section_title COLLATE NOCASE"
            .to_owned(),
        &[],
    )?;
    let mut items = courses
        .into_iter()
        .map(|course| CourseItem {
            id: course.course_id.into(),
            title: course.course_title.into(),
            section: course.section_title.into(),
            code: course.section_code.into(),
            period: course.period.unwrap_or("".to_string()).into(),
        })
        .collect::<Vec<_>>();
    let saved_order = database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = ?".to_owned(),
        &[&ORDER_KEY],
        |row| row.get::<_, String>(0).map_err(io::Error::other),
    )?;
    if let Some(order) = saved_order.first() {
        match serde_json::from_str::<Vec<String>>(order) {
            Ok(ids) => items.sort_by_key(|item| {
                ids.iter()
                    .position(|id| id == item.id.as_str())
                    .unwrap_or(usize::MAX)
            }),
            Err(error) => log::warn!("Ignoring invalid saved course order: {error}"),
        }
    }
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
        if from < 0 || from as usize >= items.len() { return; }
        let target = to.clamp(0, items.len() as i32 - 1) as usize;
        if from as usize == target { return; }
        let item = items.remove(from as usize);
        items.insert(target, item);
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        let result = serde_json::to_string(&ids).map_err(io::Error::other).and_then(|order| {
            database::execute(
                "INSERT INTO sync_state (key, data) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET data = excluded.data".to_owned(),
                &[&ORDER_KEY, &order],
            ).map(|_| ())
        });
        if let Err(error) = result {
            log::warn!("Saving course order failed: {error}");
            return;
        }
        global.set_courses(ModelRc::new(VecModel::from(items)));
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
