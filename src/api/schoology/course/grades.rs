use crate::api::schoology::account::SchoologyAccountConfig;
use std::{collections::HashMap, io};

use rusqlite::Connection;

use crate::{
    api::schoology::{RequestResult, types::grades::GradesResponse},
    database,
};

pub(crate) const UPDATE_GRADES: &str =
    "UPDATE assignments SET score = ?, letter_grade = ? WHERE course_id = ? AND id = ?";
type GradeUpdate = (Option<f64>, Option<String>, String, String);

pub(crate) fn grade_updates(
    connection: &Connection,
    response: &GradesResponse,
) -> io::Result<Vec<GradeUpdate>> {
    let mut scores = HashMap::new();
    for section in &response.section {
        let Some(course) = crate::types::course::course_from(connection, &section.section_id.0)?
        else {
            continue;
        };
        for grade in section.period.iter().flat_map(|period| &period.assignment) {
            let value = grade
                .grade
                .as_ref()
                .map(|grade| grade.0.trim())
                .filter(|grade| !grade.is_empty());
            let score = value
                .and_then(|grade| grade.parse::<f64>().ok())
                .filter(|score| score.is_finite());
            let letter = value.filter(|_| score.is_none()).map(str::to_owned);
            scores.insert(
                (course.course_id.clone(), grade.assignment_id.0.clone()),
                (score, letter),
            );
        }
    }
    Ok(scores
        .into_iter()
        .map(|((course_id, id), (score, letter))| (score, letter, course_id, id))
        .collect())
}

impl SchoologyAccountConfig {
    pub fn scrape_grades(&self) -> RequestResult<()> {
        self.apply_grades(&self.fetch_grades()?)?;
        Ok(())
    }

    pub(crate) fn fetch_grades(&self) -> RequestResult<GradesResponse> {
        let url = format!("https://api.schoology.com/v1/users/{}/grades", self.user_id);
        self.api_get(&url)
    }

    pub(crate) fn apply_grades(&self, response: &GradesResponse) -> io::Result<()> {
        let updates = {
            let connection = database::connection()?;
            grade_updates(&connection, response)?
        };
        database::bulk_execute(UPDATE_GRADES.to_owned(), updates)?;
        Ok(())
    }
}
