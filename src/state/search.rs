use serde::Deserialize;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use crate::{AppWindow, SearchResult, SearchUi, database};

#[derive(Deserialize)]
struct ResultRow {
    item_id: String,
    course_id: String,
    title: String,
    course_title: String,
    preview: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    const SCHEMA: &str = include_str!("../sql/schema.sql");

    fn insert_assignment(db: &Connection, course: &str, title: &str) {
        db.execute(
            "INSERT INTO materials VALUES (?1, 'same-id', 'assignment', '', ?2, '{}')",
            (course, title),
        )
        .unwrap();
        db.execute("INSERT INTO assignments (course_id, id, title, description, due, max_points, allow_submissions, attachments, submissions)
            VALUES (?1, 'same-id', ?2, 'Study motion', '', 10, 1, '{}', '[]')", (course, title)).unwrap();
    }

    fn results(db: &Connection, query: &str) -> Vec<ResultRow> {
        database::read_from(
            db,
            include_str!("../sql/search.sql"),
            &[&match_query(query)],
        )
        .unwrap()
    }

    #[test]
    fn fts_lifecycle_and_backfill() {
        let db = Connection::open_in_memory().unwrap();
        let base = SCHEMA
            .split("CREATE TABLE IF NOT EXISTS search_mapping")
            .next()
            .unwrap();
        db.execute_batch(base).unwrap();
        // Generate complete course fixtures from the real schema's required columns.
        let columns = db
            .prepare("SELECT name FROM pragma_table_info('courses')")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for course in ["one", "two"] {
            let values = columns
                .iter()
                .map(|column| match column.as_str() {
                    "course_id" | "course_title" => format!("'{course}'"),
                    "aliases" => "'[]'".into(),
                    _ => "''".into(),
                })
                .collect::<Vec<_>>()
                .join(",");
            db.execute(&format!("INSERT INTO courses VALUES ({values})"), [])
                .unwrap();
        }
        insert_assignment(&db, "one", "Running calculus");
        db.execute("INSERT INTO materials VALUES ('one', 'same-id', 'folder', '', 'Resources', '{\"material\":{\"body\":\"Kinematics\"}}')", []).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        assert_eq!(results(&db, "one").len(), 1);
        db.execute("UPDATE courses SET course_title = 'Mechanics', description = 'Momentum' WHERE course_id = 'one'", []).unwrap();
        assert!(results(&db, "one").is_empty());
        assert_eq!(results(&db, "mechanics").len(), 1);
        assert_eq!(results(&db, "momentum").len(), 1);
        db.execute("INSERT INTO courses SELECT 'three', aliases, 'Optics', course_code, course_url, section_title, section_code, active, period, hidden, course_order, 'Refraction', logo_img_src, location, meeting_days, start_time, end_time, weight FROM courses WHERE course_id = 'one'", []).unwrap();
        assert_eq!(results(&db, "refraction").len(), 1);
        db.execute("DELETE FROM courses WHERE course_id = 'three'", [])
            .unwrap();
        assert!(results(&db, "optics").is_empty());
        assert_eq!(results(&db, "kinematics").len(), 1);
        for kind in ["assessment", "document", "link"] {
            db.execute(
                "INSERT INTO materials VALUES ('two', 'same-id', ?1, '', 'Resources', ?2)",
                (
                    kind,
                    match kind {
                        "assessment" => r#"{"material":{"description":"Kinematics"}}"#,
                        _ => r#"{"material":{"url":"https://kinematics.example"}}"#,
                    },
                ),
            )
            .unwrap();
        }
        assert_eq!(results(&db, "kinematics").len(), 4);
        db.execute_batch(SCHEMA).unwrap();
        assert_eq!(results(&db, "kinematics").len(), 4);
        db.execute("UPDATE materials SET title = 'Reference', data = '{\"material\":{\"body\":\"Vectors\"}}', material_id = 'new-id' WHERE type = 'folder'", []).unwrap();
        assert_eq!(results(&db, "kinematics").len(), 3);
        assert_eq!(results(&db, "vectors")[0].item_id, "new-id");
        db.execute("DELETE FROM materials WHERE type = 'folder'", [])
            .unwrap();
        assert!(results(&db, "vectors").is_empty());
        assert_eq!(results(&db, "running").len(), 1);
        insert_assignment(&db, "two", "Running physics");
        db.execute_batch(SCHEMA).unwrap();
        assert_eq!(results(&db, "run").len(), 2);
        assert_eq!(results(&db, "calcu")[0].course_id, "one");
        assert_eq!(results(&db, "motion").len(), 2);
        db.execute("UPDATE assignments SET title = 'Algebra', description = 'Matrices' WHERE course_id = 'one'", []).unwrap();
        assert_eq!(results(&db, "running").len(), 1);
        assert_eq!(results(&db, "matri")[0].course_id, "one");
        db.execute("DELETE FROM materials WHERE course_id = 'two'", [])
            .unwrap();
        assert!(results(&db, "kinematics").is_empty());
        assert!(results(&db, "running").is_empty());
        db.execute("DELETE FROM courses", []).unwrap();
        assert!(results(&db, "matri").is_empty());
        assert_eq!(
            db.query_row("SELECT count(*) FROM search_mapping", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        db.execute(
            "INSERT INTO search_index(search_index) VALUES ('integrity-check')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn literal_input_is_safe_fts_syntax() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        for input in ["", "   ", "\"", "foo OR bar", "a:b", "*", "(hello)", "雪"] {
            if !match_query(input).is_empty() {
                assert!(results(&db, input).is_empty());
            }
        }
    }
}

fn match_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn search(input: &str) -> io::Result<Vec<ResultRow>> {
    let query = match_query(input);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    database::from_sql(include_str!("../sql/search.sql").into(), &[&query])
}

#[derive(Clone, Default)]
pub struct SearchState {
    generation: Arc<AtomicU64>,
}

impl SearchState {
    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let global = ui.global::<SearchUi>();
        global.invoke_search(global.get_query());
        Ok(())
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let (sender, receiver) = mpsc::channel::<(u64, String)>();
        let generation = self.generation.clone();
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            while let Ok(mut request) = receiver.recv() {
                // Coalesce typing before taking the shared database connection.
                while let Ok(next) = receiver.recv_timeout(Duration::from_millis(180)) {
                    request = next;
                }
                let (version, query) = request;
                if generation.load(Ordering::SeqCst) != version {
                    continue;
                }
                let result = search(&query);
                let generation = generation.clone();
                if weak
                    .upgrade_in_event_loop(move |ui| {
                        if generation.load(Ordering::SeqCst) != version {
                            return;
                        }
                        let global = ui.global::<SearchUi>();
                        global.set_busy(false);
                        match result {
                            Ok(rows) => {
                                global.set_error("".into());
                                global.set_results(ModelRc::new(VecModel::from(
                                    rows.into_iter()
                                        .map(|row| SearchResult {
                                            item_id: row.item_id.into(),
                                            course_id: row.course_id.into(),
                                            title: row.title.into(),
                                            course_title: row.course_title.into(),
                                            preview: scraper::Html::parse_fragment(&row.preview)
                                                .root_element()
                                                .text()
                                                .collect::<Vec<_>>()
                                                .join(" ")
                                                .into(),
                                        })
                                        .collect::<Vec<_>>(),
                                )));
                            }
                            Err(error) => {
                                log::warn!("Search failed: {error}");
                                global.set_results(ModelRc::default());
                                global.set_error("Could not search assignments. Try again.".into());
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let generation = self.generation.clone();
        let weak = ui.as_weak();
        ui.global::<SearchUi>().on_search(move |query| {
            let version = generation.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(ui) = weak.upgrade() {
                let global = ui.global::<SearchUi>();
                global.set_query(query.clone());
                global.set_results(ModelRc::default());
                global.set_error("".into());
                global.set_busy(!query.trim().is_empty());
            }
            let _ = sender.send((version, query.to_string()));
        });
    }
}
