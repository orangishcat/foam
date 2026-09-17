use std::{
    collections::BTreeMap,
    io::{self, Error},
};

use chrono::{Datelike, Days, Local};
use rusqlite::params;
use slint::{Color, ComponentHandle, ModelRc, VecModel};

use crate::{AppWindow, AssignmentCol, database, types::assignment::Assignment};

use super::courses::CourseState;

pub fn due_date_bucket(assignment: &Assignment) -> i64 {
    (assignment.due.with_timezone(&Local).date_naive() - Local::now().date_naive())
        .num_days()
        .clamp(-1, 4)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tues", "Wed", "Thu", "Fri", "Sat"];

#[derive(Default)]
pub struct DashboardState {}

impl DashboardState {
    pub fn sync_ui(&self, courses: &CourseState, ui: &AppWindow) -> io::Result<()> {
        let assignments = database::from_sql_map(
            format!(
                "SELECT a.*, COALESCE(c.course_title, 'Unknown') FROM assignments a
                WHERE NOT {} OR julianday(due) > julianday('now')
                LEFT JOIN course c
                WHERE a.course_id = c.course_id",
                include_str!("../sql/assignment/completed.sql")
            ),
            params![],
            |row| {
                Ok((
                    serde_rusqlite::from_row(row).map_err(Error::other)?,
                    row.get("course_title").map_err(Error::other)?,
                ))
            },
        )?
        .iter()
        .fold(BTreeMap::new(), |mut groups, (assignment, course_title)| {
            groups
                .entry(due_date_bucket(assignment))
                .or_insert_with(Vec::new)
                .push((assignment, course_title));
            groups
        });

        let sorted_bucket_to_modelrc = |bucket: i64| {
            let mut assign_vec = assignments.get(&bucket).cloned().unwrap_or_default();
            assign_vec.sort_by_key(|(a, _)| a.due);
            ModelRc::new(VecModel::from(
                assign_vec
                    .iter()
                    .map(|(a, course_name)| crate::Assignment {
                        title: a.title.clone().into(),
                        course_id: a.course_id.clone().into(),
                        color: if a.is_completed() {
                            Color::from_rgb_u8(70, 130, 90)
                        } else if a.is_past_due() {
                            Color::from_rgb_u8(130, 90, 60)
                        } else {
                            Color::from_rgb_u8(70, 90, 130)
                        },
                        course_name: course_name,
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

        ui.global::<crate::UiState>()
            .set_dashboard(crate::DashboardUi {
                assignment_view: ModelRc::new(VecModel::from(
                    [overdue_col, day_cols, future_col].concat(),
                )),
            });
        Ok(())
    }
}
