use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Days, Local, TimeZone, Timelike, Utc};
use rusqlite::params;
use slint::{ComponentHandle, ModelRc, ToSharedString, VecModel};

use super::state::menu_item;
use crate::{
    AppWindow, AssignmentCol, DashboardUi, NewEventUi, ScreenMenuItem,
    account::{self, Account},
    config::config,
    database, thread_manager, ui,
};
use anyhow;

pub fn due_date_bucket(due: DateTime<Utc>, day_buckets: i64) -> i64 {
    (due.with_timezone(&Local).date_naive() - Local::now().date_naive())
        .num_days()
        .clamp(-1, day_buckets)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tues", "Wed", "Thu", "Fri", "Sat"];

pub(crate) fn dashboard_query() -> String {
    format!(
        "SELECT a.*, COALESCE(c.course_title, 'Unknown') AS course_title FROM assignments a
             LEFT JOIN courses c ON a.course_id = c.course_id
             WHERE NOT ({}) OR date(due, 'localtime') >= date('now', 'localtime')",
        include_str!("../sql/assignment/completed.sql")
    )
}

#[derive(Clone, Default)]
pub struct DashboardState {}

impl crate::state::state::State for DashboardState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        let day_buckets = config().calendar_day_buckets as i64;

        let mut upcoming_assignments = database::from_sql_map::<(
            crate::types::assignment::Assignment,
            String,
        )>(dashboard_query(), params![], |row| {
            Ok((serde_rusqlite::from_row(row)?, row.get("course_title")?))
        })?
        .into_iter()
        .map(|(assignment, course_title)| (assignment, course_title, None))
        .collect::<Vec<_>>();
        // Calendar events (Schoology or user-created) are shown like assignments,
        // but have no course, completion state, or detail page.
        let mut upcoming_events = database::from_sql_map(
            "SELECT e.*, COALESCE(c.course_title, '') AS course_title FROM events e
                 LEFT JOIN courses c ON e.course_id = c.course_id
                 WHERE date(e.due, 'localtime') >= date('now', 'localtime')"
                .to_owned(),
            params![],
            |row| {
                Ok((
                    serde_rusqlite::from_row::<crate::types::event::Event>(row)?,
                    row.get::<_, String>("course_title")?,
                ))
            },
        )?;
        upcoming_assignments.extend(upcoming_events.drain(..).map(|(event, course_title)| {
            let subtitle = [event.subtitle, course_title]
                .into_iter()
                .find(|subtitle| !subtitle.is_empty())
                .unwrap_or_else(|| "Event".to_owned());
            (
                crate::types::assignment::Assignment {
                    id: event.id,
                    title: event.title,
                    description: event.description,
                    due: event.due,
                    ..Default::default()
                },
                subtitle,
                Some(event.source),
            )
        }));
        let assignments_on_dashboard = upcoming_assignments.into_iter().fold(
            BTreeMap::new(),
            |mut groups, (assignment, course_title, event_source)| {
                groups
                    .entry(due_date_bucket(assignment.due, day_buckets))
                    .or_insert_with(Vec::new)
                    .push((assignment, course_title, event_source));
                groups
            },
        );

        let sorted_bucket_to_modelrc = |bucket: i64| {
            let mut assign_vec = assignments_on_dashboard
                .get(&bucket)
                .cloned()
                .unwrap_or_default();
            assign_vec.sort_by_key(|(a, _, _)| a.due);
            ModelRc::new(VecModel::from(
                assign_vec
                    .iter()
                    .map(|(a, course_name, event_source)| crate::Assignment {
                        id: a.id.to_shared_string(),
                        title: a.title.clone().into(),
                        course_id: a.course_id.clone().into(),
                        course_name: course_name.to_shared_string(),
                        date: a
                            .due
                            .with_timezone(&Local)
                            .format("%b %-d")
                            .to_shared_string(),
                        time: a
                            .due
                            .with_timezone(&Local)
                            .format("%I:%M %p")
                            .to_shared_string(),
                        done: a.is_completed(),
                        overdue: a.is_past_due(),
                        is_event: event_source.is_some(),
                        is_schoology: event_source.as_deref() == Some("schoology"),
                    })
                    .collect::<Vec<crate::Assignment>>(),
            ))
        };

        let overdue_col = vec![crate::AssignmentCol {
            title: "Overdue".into(),
            assignments: sorted_bucket_to_modelrc(-1),
        }];
        let day_cols = (0..day_buckets as u64)
            .map(|day_add| crate::AssignmentCol {
                title: Local::now()
                    .checked_add_days(Days::new(day_add))
                    .map_or_else(
                        || format!("{day_add} days later"),
                        |date| {
                            WEEKDAY_NAMES[date.weekday().num_days_from_sunday() as usize].to_owned()
                        },
                    )
                    .into(),
                assignments: sorted_bucket_to_modelrc(day_add as i64),
            })
            .collect::<Vec<AssignmentCol>>();
        let future_col = vec![crate::AssignmentCol {
            title: "Future".into(),
            assignments: sorted_bucket_to_modelrc(day_buckets),
        }];
        let g = ui.global::<crate::DashboardUi>();
        g.set_assignment_view(ModelRc::new(VecModel::from(
            [overdue_col, day_cols, future_col].concat(),
        )));
        g.on_open_event_in_browser(|id| {
            let result = (|| -> anyhow::Result<String> {
                let account = account::active_account()?;
                account
                    .event_url(&id)
                    .ok_or_else(|| anyhow::anyhow!("Event has no browser URL"))
            })();
            match result {
                Ok(url) => crate::state::attachment_view::open_browser(&url),
                Err(error) => log::warn!("Opening event {id} in browser failed: {error}"),
            }
        });
        g.on_set_done(|done, id| {
            database::execute(
                include_str!("../sql/assignment/set-done.sql").to_owned(),
                params![done, id.to_string()],
            )
            .inspect_err(|err| log::warn!("Error marking assignment {id} as done: {err}"))
            .ok();
        });
        g.set_column_count(config().calendar_day_buckets + 2);

        let window = ui.as_weak();
        g.on_new_event(move || {
            let Some(ui) = window.upgrade() else { return };
            let dialog = ui.global::<NewEventUi>();
            let now = Local::now();
            dialog.set_date(crate::Date {
                year: now.year(),
                month: now.month() as i32,
                day: now.day() as i32,
            });
            dialog.set_time(crate::Time {
                hour: 8,
                minute: 0,
                second: 0,
            });
            dialog.set_title(Default::default());
            dialog.set_subtitle(Default::default());
            dialog.set_description(Default::default());
            dialog.set_editing_id(Default::default());
            dialog.set_read_only(false);
            dialog.set_visible(true);
        });
        let window = ui.as_weak();
        g.on_edit_event(move |assignment| {
            let Some(ui) = window.upgrade() else { return };
            let id = assignment.id.to_string();
            let event = database::from_sql_map(
                "SELECT * FROM events WHERE id = ?".to_owned(),
                params![id],
                |row| Ok(serde_rusqlite::from_row::<crate::types::event::Event>(row)?),
            )
            .inspect_err(|err| log::warn!("Error loading event {id} for editing: {err}"))
            .ok()
            .and_then(|mut events| events.pop());
            let Some(event) = event else { return };
            // Schoology events are managed by the calendar feed, so they open read-only.
            let due = event.due.with_timezone(&Local);
            let dialog = ui.global::<NewEventUi>();
            dialog.set_read_only(event.source != "user");
            dialog.set_editing_id(event.id.clone().into());
            dialog.set_title(event.title.clone().into());
            dialog.set_subtitle(event.subtitle.into());
            dialog.set_description(event.description.into());
            dialog.set_date(crate::Date {
                year: due.year(),
                month: due.month() as i32,
                day: due.day() as i32,
            });
            dialog.set_time(crate::Time {
                hour: due.hour() as i32,
                minute: due.minute() as i32,
                second: 0,
            });
            dialog.set_visible(true);
        });
        let window = ui.as_weak();
        let dialog = ui.global::<NewEventUi>();
        dialog.on_create(
            move |title, subtitle, description, year, month, day, hour, minute| {
                if title.trim().is_empty() {
                    return;
                }
                let Some(ui) = window.upgrade() else { return };
                if ui.global::<NewEventUi>().get_read_only() {
                    return;
                }
                let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                else {
                    log::warn!("Ignoring event with invalid date {year}-{month}-{day}");
                    return;
                };
                let Some(naive) = date.and_hms_opt(hour as u32, minute as u32, 0) else {
                    log::warn!("Ignoring event with invalid time {hour}:{minute}");
                    return;
                };
                let Some(due) = Local.from_local_datetime(&naive).single() else {
                    log::warn!("Ignoring event with ambiguous local time");
                    return;
                };
                let editing_id = ui.global::<NewEventUi>().get_editing_id().to_string();
                let id = if editing_id.is_empty() {
                    format!(
                        "user-{}",
                        Utc::now().timestamp_nanos_opt().unwrap_or_default()
                    )
                } else {
                    editing_id
                };
                let event = crate::types::event::Event {
                    id,
                    title: title.to_string(),
                    subtitle: subtitle.to_string(),
                    description: description.to_string(),
                    due: due.with_timezone(&Utc),
                    source: "user".to_owned(),
                    course_id: None,
                };
                let result = database::connection()
                    .and_then(|connection| event.upsert_on(&connection))
                    .inspect_err(|err| log::warn!("Error saving event: {err}"));
                if result.is_ok() {
                    ui.global::<NewEventUi>().set_visible(false);
                    ui::sync_ui();
                }
            },
        );
        let window = ui.as_weak();
        ui.global::<NewEventUi>().on_delete(move || {
            let Some(ui) = window.upgrade() else { return };
            let dialog = ui.global::<NewEventUi>();
            let id = dialog.get_editing_id().to_string();
            if id.is_empty() || dialog.get_read_only() {
                return;
            }
            let deleted = database::execute(
                include_str!("../sql/event/delete.sql").to_owned(),
                params![id],
            )
            .inspect_err(|err| log::warn!("Error deleting event {id}: {err}"));
            if deleted.is_ok_and(|count| count > 0) {
                dialog.set_visible(false);
                ui::sync_ui();
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
        let selected = ui.global::<DashboardUi>().get_selected_assignment();
        let enabled = !selected.id.is_empty();
        vec![
            menu_item("View item", &["V"], enabled),
            menu_item(
                if selected.done {
                    "Unmark as done"
                } else {
                    "Mark as done"
                },
                &["D"],
                enabled && !selected.is_event,
            ),
            menu_item("Open in browser", &["O"], selected.is_schoology),
            menu_item("Unselect item", &["Escape"], enabled),
            menu_item("Refresh", &["Control", "R"], true),
            menu_item("New event", &["Control", "N"], true),
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
