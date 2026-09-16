use std::collections::HashMap;

use crate::{
    api::schoology::{RequestResult, api_get, types::grades::GradesResponse},
    config::config,
    types::{course::Course, folder::Folder, material::Material},
};

// Schoology's user endpoint nests assignment grades under section -> period -> assignment.
// Section totals (`final_grade`) are deliberately not assignment scores.
// https://developers.schoology.com/api-documentation/rest-api-v1/user-grades/

/// Populate scores for the configured user's assignments, matched within each section.
pub fn scrape_grades(courses: &mut [Course]) -> RequestResult<()> {
    apply(courses, &fetch()?);
    Ok(())
}

pub(crate) fn fetch() -> RequestResult<GradesResponse> {
    let url = format!(
        "https://api.schoology.com/v1/users/{}/grades",
        config().user_id
    );
    api_get(&url)
}

pub(crate) fn apply(courses: &mut [Course], response: &GradesResponse) {
    for course in courses {
        apply_grades(course, response);
    }
}

fn apply_grades(course: &mut Course, response: &GradesResponse) {
    let scores: HashMap<String, Option<String>> = response
        .section
        .iter()
        .filter(|section| {
            section.section_id.0 == course.course_id
                || course.aliases.contains(&section.section_id.0)
        })
        .flat_map(|section| &section.period)
        .flat_map(|period| &period.assignment)
        .map(|grade| {
            (
                grade.assignment_id.0.clone(),
                grade
                    .grade
                    .as_ref()
                    .map(|grade| grade.0.clone())
                    .filter(|grade| !grade.trim().is_empty()),
            )
        })
        .collect();
    set_scores(&mut course.materials, &scores);
}

fn set_scores(folder: &mut Folder, scores: &HashMap<String, Option<String>>) {
    for material in &mut folder.materials {
        match material {
            Material::Folder(folder) => set_scores(folder, scores),
            Material::Assignment(assignment) => {
                let grade = scores
                    .get(&assignment.id)
                    .cloned()
                    .flatten()
                    .inspect(|score| {
                        log::debug!(
                            "Applied grade {}/{} to assignment id {} (\"{}\")",
                            score,
                            assignment.max_points,
                            assignment.id,
                            assignment.title,
                        )
                    });
                assignment.score = grade
                    .as_deref()
                    .and_then(|grade| grade.trim().parse::<f64>().ok())
                    .filter(|score| score.is_finite());
                assignment.letter_grade = grade.filter(|_| assignment.score.is_none());
            }
            _ => {}
        }
    }
}
