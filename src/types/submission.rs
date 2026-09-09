use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
