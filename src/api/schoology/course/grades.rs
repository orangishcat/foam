use std::collections::HashMap;

use log::info;

use crate::{
    api::schoology::{
        RequestResult, api_get_with_query,
        types::grades::{GradesQuery, GradesResponse},
    },
    config::config,
    types::{course::Course, folder::Folder, material::Material},
};

// Schoology's user endpoint nests assignment grades under section -> period -> assignment.
// Section totals (`final_grade`) are deliberately not assignment scores.
// https://developers.schoology.com/api-documentation/rest-api-v1/user-grades/

/// Populate scores for the configured user's assignments, matched within each section.
pub fn scrape_grades(courses: &mut [Course]) -> RequestResult<()> {
    let url = format!(
        "https://api.schoology.com/v1/users/{}/grades",
        config().user_id
    );
    for course in courses {
        // The default endpoint returns active enrollments and supports section_id.
        // Pagination by enrollment is documented for timestamp/include_all_enrollments;
        // neither is requested here. No grading-period filter means all periods.
        // https://developers.schoology.com/api-documentation/rest-api-v1/user-grades/
        let response: GradesResponse = api_get_with_query(
            &url,
            &GradesQuery {
                section_id: &course.course_id,
            },
        )?;
        apply_grades(course, response);
    }
    Ok(())
}

fn apply_grades(course: &mut Course, response: GradesResponse) {
    let scores: HashMap<String, Option<String>> = response
        .section
        .into_iter()
        .filter(|section| section.section_id.0 == course.course_id)
        .flat_map(|section| section.period)
        .flat_map(|period| period.assignment)
        .map(|grade| {
            (
                grade.assignment_id.0,
                grade
                    .grade
                    .map(|grade| grade.0)
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
                        info!(
                            "Applied grade {}/{} to assignment id {} (\"{}\")",
                            score, assignment.max_points, assignment.id, assignment.title,
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
