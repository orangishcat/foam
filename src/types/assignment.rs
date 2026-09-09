use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{attachment::Attachments, submission::Submission};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assignment {
    #[serde(default)]
    pub course_id: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due: DateTime<Utc>,
    #[serde(default)]
    pub max_points: f64,
    /// Recorded numeric score; absent for letter grades or ungraded assignments.
    #[serde(default)]
    pub score: Option<f64>,
    /// Recorded nonnumeric grade, such as `A-`.
    #[serde(default)]
    pub letter_grade: Option<String>,
    #[serde(default)]
    pub allow_submissions: bool,
    #[serde(default)]
    pub attachments: Attachments,
    /// The configured user's submission revisions, including drafts.
    #[serde(default)]
    pub submissions: Vec<Submission>,
}
