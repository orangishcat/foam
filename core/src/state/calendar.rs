use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::params;
use slint::{ComponentHandle, ModelRc, ToSharedString, VecModel};

use super::state::menu_item;
use crate::{
    AppWindow, CalendarUi, ScreenMenuItem, TaskCol,
    account::{self, Account},
    database, thread_manager, types::task::Task, ui,
};
use anyhow;

pub fn due_date_bucket(task: &Task) -> i64 {
    (task.due.with_timezone(&Local).date_naive() - Local::now().date_naive())
        .num_days()
        .clamp(-1, 4)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tues", "Wed", "Thu", "Fri", "Sat"];

pub(crate) fn calendar_query() -> String {
    // Tasks without a due date are excluded; every source shows up.
    format!(
        "SELECT a.*, COALESCE(ts.completed, 0) AS completed, COALESCE(f.title, 'Unknown') AS folder_title FROM tasks a
             LEFT JOIN folders f ON a.folder_id = f.folder_id
             LEFT JOIN task_state ts ON ts.folder_id = a.folder_id AND ts.task_id = a.id
             WHERE due IS NOT NULL"
    )
}

#[derive(Clone, Default)]
pub struct CalendarState {}

impl crate::state::state::State for CalendarState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        let tasks_on_calendar =
            database::from_sql_map::<(Task, String, bool)>(calendar_query(), params![], |row| {
                Ok((
                    serde_rusqlite::from_row(row)?,
                    row.get("folder_title")?,
                    row.get("completed")?,
                ))
            })?
            .into_iter()
            .fold(BTreeMap::new(), |mut groups, (task, folder_title, completed)| {
                groups
                    .entry(due_date_bucket(&task))
                    .or_insert_with(Vec::new)
                    .push((task, folder_title, completed));
                groups
            });

        let sorted_bucket_to_modelrc = |bucket: i64| {
            let mut task_vec = tasks_on_calendar
                .get(&bucket)
                .cloned()
                .unwrap_or_default();
            task_vec.sort_by_key(|(a, _, _)| a.due);
            ModelRc::new(VecModel::from(
                task_vec
                    .iter()
                    .map(|(a, folder_name, completed)| crate::TaskItem {
                        id: a.id.to_shared_string(),
                        title: a.title.clone().into(),
                        folder_id: a.folder_id.clone().into(),
                        folder_name: folder_name.to_shared_string(),
                        overdue: a.is_past_due(),
                        completed: *completed,
                    })
                    .collect::<Vec<crate::TaskItem>>(),
            ))
        };

        let overdue_col = vec![TaskCol {
            title: "Overdue".into(),
            tasks: sorted_bucket_to_modelrc(-1),
        }];
        let day_cols = (0..4)
            .map(|day_add| TaskCol {
                title: Local::now()
                    .checked_add_days(Days::new(day_add))
                    .map_or_else(
                        || format!("{day_add} days later"),
                        |date| {
                            WEEKDAY_NAMES[date.weekday().num_days_from_sunday() as usize].to_owned()
                        },
                    )
                    .into(),
                tasks: sorted_bucket_to_modelrc(day_add as i64),
            })
            .collect::<Vec<TaskCol>>();
        let future_col = vec![TaskCol {
            title: "Future".into(),
            tasks: sorted_bucket_to_modelrc(4),
        }];
        let g = ui.global::<crate::CalendarUi>();
        g.set_task_view(ModelRc::new(VecModel::from(
            [overdue_col, day_cols, future_col].concat(),
        )));
        folder_options(ui);
        let weak = ui.as_weak();
        g.on_create_event(move |title, description, due, folder_index| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<crate::CalendarUi>();
            match create_event(&title, &description, &due, folder_index) {
                Ok(()) => {
                    global.set_create_visible(false);
                    ui.global::<crate::CalendarUi>()
                        .set_create_error("".into());
                }
                Err(err) => {
                    log::warn!("Creating event failed: {err}");
                    global.set_create_error(format!("{err}").into());
                }
            }
        });
        let weak = ui.as_weak();
        g.on_toggle_completed(move |folder_id, task_id| {
            let result = database::execute(
                "INSERT INTO task_state (folder_id, task_id, completed) VALUES (?1, ?2, 1)
                 ON CONFLICT(folder_id, task_id) DO UPDATE SET completed = NOT task_state.completed"
                    .to_owned(),
                params![folder_id.to_string(), task_id.to_string()],
            );
            match result {
                Ok(_) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.global::<CalendarUi>().set_selected_task(Default::default());
                        ui::sync_ui();
                    }
                }
                Err(error) => log::warn!("Toggling task completion failed: {error}"),
            }
        });
        g.on_refresh(|| {
            account::active_account()
                .map(|acc| {
                    thread_manager::spawn_thread("Manual refresh", move || {
                        manual_refresh(acc)
                            .inspect_err(|err| log::warn!("Manual refresh failed: {err}"))
                            .ok();
                        ui::sync_ui();
                    })
                    .inspect_err(|err| log::warn!("Manual refresh thread spawning failed: {err}"))
                    .ok();
                })
                .inspect_err(|err| log::warn!("Error manual refreshing: {err}"))
                .ok();
        });
        Ok(())
    }

    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        let selected = ui.global::<CalendarUi>().get_selected_task();
        let enabled = !selected.folder_id.is_empty();
        vec![
            menu_item("View item", &["V"], enabled),
            menu_item("Unselect item", &["Escape"], enabled),
            menu_item("Refresh", &["Control", "R"], true),
        ]
    }
}

fn manual_refresh(acc: Box<dyn Account>) -> anyhow::Result<()> {
    acc.fast_sync()?;
    log::info!("Manually refreshed fast sync");
    acc.slow_sync()?;
    log::info!("Manually refreshed slow sync");
    Ok(())
}

/// Folder selector options for the create-event dialog; the create handler
/// re-runs the same ordered query so an option index maps back to a folder id.
fn folder_options(ui: &AppWindow) {
    let options = database::from_sql_map(
        "SELECT title FROM folders ORDER BY title COLLATE NATURAL_ORDER, folder_id"
            .to_owned(),
        &[],
        |row| Ok(row.get::<_, String>(0)?),
    )
    .unwrap_or_default();
    ui.global::<crate::CalendarUi>()
        .set_folder_options(ModelRc::new(VecModel::from(
            options.iter().map(ToSharedString::to_shared_string).collect::<Vec<_>>(),
        )));
}

fn parse_due(value: &str) -> anyhow::Result<DateTime<Utc>> {
    let value = value.trim();
    let naive = if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        date.and_hms_opt(23, 59, 0).expect("fixed time is valid")
    } else {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")?
    };
    // interpret as local time; on DST-gap ambiguity take the earliest instant,
    // and fall back to treating the input as UTC when unresolvable
    Ok(match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(value) => value.with_timezone(&Utc),
        chrono::LocalResult::Ambiguous(value, _) => value.with_timezone(&Utc),
        chrono::LocalResult::None => Utc.from_utc_datetime(&naive),
    })
}

/// Inserts a builtin task (with `source = 'builtin'`) so the calendar picks it
/// up from its ordinary due-based listing. `due` may be empty (all-day event
/// with no deadline) or `YYYY-MM-DD` / `YYYY-MM-DD HH:MM` local time.
fn create_event(
    title: &str,
    description: &str,
    due: &str,
    folder_index: i32,
) -> anyhow::Result<()> {
    if title.trim().is_empty() {
        anyhow::bail!("Title is required");
    }
    let folders = database::from_sql_map(
        "SELECT folder_id, title FROM folders ORDER BY title COLLATE NATURAL_ORDER, folder_id"
            .to_owned(),
        &[],
        |row| Ok(row.get::<_, String>(0)?),
    )?;
    let folder_id = folders
        .get(folder_index.max(0) as usize)
        .ok_or_else(|| anyhow::anyhow!("No folder selected"))?;
    let parsed_due = if due.trim().is_empty() {
        None
    } else {
        Some(parse_due(due)?)
    };
    let id = format!("builtin-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    let root: String = {
        let mut rows = transaction
            .prepare_cached("SELECT resource_id FROM resources WHERE folder_id = ? AND type = 'folder' AND parent_id = ''")?
            .query_map(params![folder_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.pop().unwrap_or_else(|| "0".to_owned())
    };
    transaction.execute(
        "INSERT INTO resources (folder_id, resource_id, type, parent_id, source, title, data)
         VALUES (?1, ?2, 'task', ?3, 'builtin', ?4, '{}')
         ON CONFLICT(folder_id, type, resource_id) DO NOTHING",
        params![folder_id, id, root, title],
    )?;
    transaction.execute(
        "INSERT INTO tasks (folder_id, id, source, title, description, due, attachments)
         VALUES (?1, ?2, 'builtin', ?3, ?4, ?5, '{}')
         ON CONFLICT(folder_id, id) DO NOTHING",
        params![folder_id, id, title, description, parsed_due.map(|dt| dt.to_rfc3339())],
    )?;
    transaction.commit()?;
    log::info!("Created event {id} in folder {folder_id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_loads_native_completion_state() {
        let connection = rusqlite::Connection::open_in_memory().expect("open test database");
        connection
            .execute_batch(include_str!("../sql/schema.sql"))
            .expect("create test schema");
        connection
            .execute(
                "INSERT INTO folders (folder_id, title, data) VALUES ('f', 'Folder', '{}')",
                [],
            )
            .expect("create folder");
        connection
            .execute(
                "INSERT INTO resources (folder_id, resource_id, type, parent_id, title, data)
                 VALUES ('f', 't', 'task', '', 'Task', '{}')",
                [],
            )
            .expect("create task resource");
        connection
            .execute(
                "INSERT INTO tasks (folder_id, id, title, description, due, attachments)
                 VALUES ('f', 't', 'Task', '', '2026-11-09T12:00:00Z', '{}')",
                [],
            )
            .expect("create task");
        connection
            .execute(
                "INSERT INTO task_state (folder_id, task_id, completed) VALUES ('f', 't', 1)",
                [],
            )
            .expect("mark task completed");

        let mut statement = connection
            .prepare(&calendar_query())
            .expect("prepare calendar query");
        let mut rows = statement.query([]).expect("query calendar task");
        let row = rows.next().expect("read row").expect("task exists");
        let task = serde_rusqlite::from_row::<Task>(row).expect("deserialize task");
        assert_eq!(task.title, "Task");
        assert_eq!(row.get::<_, String>("folder_title").unwrap(), "Folder");
        assert!(row.get::<_, bool>("completed").unwrap());
    }
}
