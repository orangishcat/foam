use crate::types::{LooseInt, attachment::Attachments};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub course_fid: i64,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub published: bool,
    #[serde(default)]
    pub attachments: Attachments,
    #[serde(default)]
    pub display_inline: LooseInt,
}
