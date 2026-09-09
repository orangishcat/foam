use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::attachment::Attachments;

/// One submission revision, scoped to its containing assignment and user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Submission {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub late: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub attachments: Attachments,
}
