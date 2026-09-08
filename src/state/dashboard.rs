use std::{
    cmp::{max, min},
    collections::BTreeMap,
};

use chrono::{DateTime, Datelike, Days, Local, Utc};
use slint::{Color, ModelRc, VecModel};

use crate::{
    AssignmentCol,
    types::{
        assignment::{self, Assignment},
        material::Material,
    },
};

use super::courses::CourseState;

pub fn due_date_bucket(assignment: &Assignment) -> i64 {
    (assignment.due - Utc::now()).num_days().clamp(-1, 4)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tues", "Wed", "Thu", "Fri", "Sat"];

#[derive(Default)]
pub struct DashboardState {}

impl DashboardState {
    pub fn sync_ui<'a>(&'a self, courses: &CourseState, ui: &crate::AppWindow) {
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

        let now = Local::now();
        let sorted_bucket_to_modelrc = |bucket: i64| {
            let mut assign_vec = assignments.get(&bucket).cloned().unwrap_or_default();
            assign_vec.sort_by_key(|a| a.due);
            ModelRc::new(VecModel::from(
                assign_vec
                    .iter()
                    .map(|a| crate::Assignment {
                        title: a.title.clone().into(),
                        course_id: a.course_id.clone().into(),
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
            color: slint::Color::from_rgb_u8(130, 90, 60),
        }];
        let day_cols = (0..3)
            .map(|day_add| crate::AssignmentCol {
                title: now
                    .checked_add_days(Days::new(day_add))
                    .map_or_else(
                        || format!("{day_add} days later"),
                        |date| {
                            WEEKDAY_NAMES[date.weekday().num_days_from_sunday() as usize].to_owned()
                        },
                    )
                    .into(),
                assignments: sorted_bucket_to_modelrc(day_add as i64),
                color: Color::from_rgb_u8(70, 90, 130),
            })
            .collect::<Vec<AssignmentCol>>();
        let future_col = vec![crate::AssignmentCol {
            title: "Future".into(),
            assignments: sorted_bucket_to_modelrc(4),
            color: slint::Color::from_rgb_u8(130, 90, 60),
        }];

        ui.set_assignment_view(ModelRc::new(VecModel::from(
            [overdue_col, day_cols, future_col].concat(),
        )));
    }
}
