use crate::{
    AppWindow, CourseItem, CoursesUi,
    api::schoology::{self, RequestResult},
    database,
    types::course::Course,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::io;

pub fn sync_ui(ui: &AppWindow) -> io::Result<()> {
    let courses = database::from_sql::<Course>(
        "SELECT * FROM courses ORDER BY course_title COLLATE NOCASE, section_title COLLATE NOCASE"
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
        })
        .collect::<Vec<_>>();
    ui.global::<CoursesUi>()
        .set_courses(ModelRc::new(VecModel::from(items)));
    Ok(())
}

/// Retry an interrupted initial fetch even if some courses were already saved.
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
