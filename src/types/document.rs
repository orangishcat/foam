use crate::types::{LooseInt, attachment::Attachments};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub course_fid: LooseInt,
    #[serde(default)]
    pub attachments: Attachments,
}
