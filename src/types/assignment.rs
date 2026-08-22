use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::attachment::Attachments;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assignment {
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
    #[serde(default)]
    pub allow_submissions: bool,
    #[serde(default)]
    pub attachments: Attachments,
}
