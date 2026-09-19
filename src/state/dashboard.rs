use std::{
    collections::BTreeMap,
    io::{self, Error},
};

use chrono::{Datelike, Days, Local};
use rusqlite::params;
use slint::{ComponentHandle, ModelRc, ToSharedString, VecModel};

use crate::{AppWindow, AssignmentCol, database, types::assignment::Assignment};

pub fn due_date_bucket(assignment: &Assignment) -> i64 {
    (assignment.due.with_timezone(&Local).date_naive() - Local::now().date_naive())
        .num_days()
        .clamp(-1, 4)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tues", "Wed", "Thu", "Fri", "Sat"];

pub(crate) fn dashboard_query() -> String {
    format!(
        "SELECT a.*, COALESCE(c.course_title, 'Unknown') AS course_title FROM assignments a
             LEFT JOIN courses c ON a.course_id = c.course_id
             WHERE NOT ({}) OR julianday(due) > julianday('now')",
        include_str!("../sql/assignment/completed.sql")
    )
}

#[derive(Clone, Default)]
pub struct DashboardState {}

impl DashboardState {
    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let assignments_on_dashboard =
            database::from_sql_map::<(Assignment, String)>(dashboard_query(), params![], |row| {
                Ok((
                    serde_rusqlite::from_row(row).map_err(Error::other)?,
                    row.get("course_title").map_err(Error::other)?,
                ))
            })?
            .into_iter()
            .fold(BTreeMap::new(), |mut groups, (assignment, course_title)| {
                groups
                    .entry(due_date_bucket(&assignment))
                    .or_insert_with(Vec::new)
                    .push((assignment, course_title));
                groups
            });

        let sorted_bucket_to_modelrc = |bucket: i64| {
            let mut assign_vec = assignments_on_dashboard
                .get(&bucket)
                .cloned()
                .unwrap_or_default();
            assign_vec.sort_by_key(|(a, _)| a.due);
            ModelRc::new(VecModel::from(
                assign_vec
                    .iter()
                    .map(|(a, course_name)| crate::Assignment {
                        id: a.id.to_shared_string(),
                        title: a.title.clone().into(),
                        course_id: a.course_id.clone().into(),
                        course_name: course_name.to_shared_string(),
                        done: a.is_completed(),
                        overdue: a.is_past_due(),
                    })
                    .collect::<Vec<crate::Assignment>>(),
            ))
        };

        let overdue_col = vec![crate::AssignmentCol {
            title: "Overdue".into(),
            assignments: sorted_bucket_to_modelrc(-1),
        }];
        let day_cols = (0..4)
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
            assignments: sorted_bucket_to_modelrc(4),
        }];
        let g = ui.global::<crate::DashboardUi>();
        g.set_assignment_view(ModelRc::new(VecModel::from(
            [overdue_col, day_cols, future_col].concat(),
        )));
        g.on_set_done(|done, id| {
            database::execute(
                include_str!("../sql/assignment/set-done.sql").to_owned(),
                params![done, id.to_string()],
            )
            .inspect_err(|err| log::warn!("Error marking assignment {id} as done: {err}"))
            .ok();
        });
        Ok(())
    }
}
