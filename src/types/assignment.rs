use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{PickFirst, Same, json::JsonString, serde_as};

use crate::types::{attachment::Attachments, submission::Submission};

#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Assignment {
    pub course_id: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub due: DateTime<Utc>,
    pub manual_mark: Option<bool>, // true / false / (none -> fallback to other checks)
    pub max_points: f64,
    pub score: Option<f64>,
    pub letter_grade: Option<String>, // why on earth does schoology allow letter grades
    pub allow_submissions: bool,
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub attachments: Attachments,
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub submissions: Vec<Submission>, // schoology revisions + drafts
}

impl Assignment {
    pub(crate) fn insert_on(
        &self,
        connection: &rusqlite::Connection,
        course_id: &str,
    ) -> std::io::Result<()> {
        let mut assignment = self.clone();
        assignment.course_id = course_id.to_owned();
        let serialized =
            serde_rusqlite::to_params_named(&assignment).map_err(std::io::Error::other)?;
        connection
            .execute(
                include_str!("../sql/assignment/write.sql"),
                serialized.to_slice().as_slice(),
            )
            .map_err(std::io::Error::other)?;
        Ok(())
    }
    pub fn is_completed(&self) -> bool {
        if let Some(mark) = self.manual_mark {
            return mark;
        }
        (self.is_past_due() && !self.allow_submissions)
            || !self.submissions.is_empty()
            || self.is_scored()
    }
    pub fn is_past_due(&self) -> bool {
        self.due < Utc::now()
    }
    pub fn is_scored(&self) -> bool {
        self.score.is_some() || self.letter_grade.is_some()
    }
}
