use crate::types::attachment::Attachments;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub url: String,
    pub course_fid: i64,
    pub attachments: Attachments,
}
