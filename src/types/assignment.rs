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
    pub manual_mark: Option<bool>, // true / false / (none -> fallback to other checks)
    pub max_points: f64,
    pub score: Option<f64>,
    pub letter_grade: Option<String>, // why on earth does schoology allow letter grades
    pub allow_submissions: bool,
    pub attachments: Attachments,
    pub submissions: Vec<Submission>, // schoology revisions + drafts
}

impl Assignment {
    pub fn is_completed(&self) -> bool {
        if let Some(mark) = self.manual_mark {
            return mark;
        }
        (self.is_overdue() && !self.allow_submissions) || !self.submissions.is_empty()
    }
    pub fn is_overdue(&self) -> bool {
        self.due < Utc::now()
    }
}
