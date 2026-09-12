use std::collections::BTreeMap;

use chrono::{Datelike, Days, Local, Utc};
use slint::{Color, ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow, AssignmentCol,
    types::{assignment::Assignment, material::Material},
};

use super::courses::CourseState;

const EXCLUDED_FROM_DASHBOARD: i64 = -5;

pub fn due_date_bucket(assignment: &Assignment) -> i64 {
    if assignment.due < Utc::now()
        && (assignment.score.is_some()
            || assignment.letter_grade.is_some()
            || !assignment.allow_submissions
            || !assignment.submissions.is_empty())
    {
        return EXCLUDED_FROM_DASHBOARD;
    }
    (assignment.due - Utc::now()).num_days().clamp(-1, 4)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tues", "Wed", "Thu", "Fri", "Sat"];

#[derive(Default)]
pub struct DashboardState {}

impl DashboardState {
    pub fn sync_ui(&self, courses: &CourseState, ui: &AppWindow) {
        let assignments = courses
            .walk_materials()
            .filter_map(|material| match material {
                Material::Assignment(assignment) => Some(assignment),
                _ => None,
            })
            .fold(BTreeMap::new(), |mut groups, assignment| {
                groups
                    .entry(due_date_bucket(assignment))
                    .or_insert_with(Vec::new)
                    .push(assignment);
                groups
            });

        let sorted_bucket_to_modelrc = |bucket: i64| {
            let mut assign_vec = assignments.get(&bucket).cloned().unwrap_or_default();
            assign_vec.sort_by_key(|a| a.due);
            ModelRc::new(VecModel::from(
                assign_vec
                    .iter()
                    .map(|a| crate::Assignment {
                        title: a.title.clone().into(),
                        course_id: a.course_id.clone().into(),
                        color: if a.is_completed() {
                            Color::from_rgb_u8(70, 130, 90)
                        } else if a.is_overdue() {
                            Color::from_rgb_u8(130, 90, 60)
                        } else {
                            Color::from_rgb_u8(70, 90, 130)
                        },
                        course_name: courses
                            .get_course(&a.course_id)
                            .map(|c| c.course_title.as_str())
                            .unwrap_or("Unknown")
                            .into(),
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
    }
}
