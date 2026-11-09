use crate::types::attachment::Attachments;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    pub id: String,
    pub title: String,
    pub url: String,
    pub course_fid: i64,
    pub available: bool,
    pub published: bool,
    pub attachments: Attachments,
    pub display_inline: bool,
}
