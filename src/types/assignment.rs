use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{attachment::Attachments, submission::Submission};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Assignment {
    pub course_id: String,
    pub id: String,
    pub title: String,
    pub description: String,
    pub due: DateTime<Utc>,
    pub max_points: f64,
    /// Recorded numeric score; absent for letter grades or ungraded assignments.
    pub score: Option<f64>,
    /// Recorded nonnumeric grade, such as `A-`.
    pub letter_grade: Option<String>,
    pub allow_submissions: bool,
    pub attachments: Attachments,
    /// The configured user's submission revisions, including drafts.
    pub submissions: Vec<Submission>,
}
