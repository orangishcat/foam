use super::assignment::Assignment;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{Error, Result};

use super::attachment::Attachments;

/// One submission revision, scoped to its containing assignment and user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Submission {
    pub id: String,
    pub user_id: String,
    pub created: DateTime<Utc>,
    pub late: bool,
    pub draft: bool,
    pub body: String,
    pub attachments: Attachments,
}

pub fn update_submissions(assignments: &[Assignment]) -> Result<usize> {
    crate::database::bulk_execute(
        UPDATE_SUBMISSIONS.to_owned(),
        submission_updates(assignments)?,
    )
}

pub(crate) const UPDATE_SUBMISSIONS: &str =
    "UPDATE assignments SET submissions = ? WHERE course_id = ? AND id = ?";

pub(crate) fn submission_updates(assignments: &[Assignment]) -> Result<Vec<(String, &str, &str)>> {
    assignments
        .iter()
        .map(|a| {
            Ok((
                serde_json::to_string(&a.submissions).map_err(Error::other)?,
                a.course_id.as_str(),
                a.id.as_str(),
            ))
        })
        .collect()
}
