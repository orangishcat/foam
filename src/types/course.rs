use serde::{Deserialize, Serialize};

use super::folder::Folder;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Course {
    #[serde(default)]
    pub course_id: String,
    #[serde(default)]
    pub course_title: String,
    #[serde(default)]
    pub course_code: String,
    #[serde(default)]
    pub course_url: String,
    #[serde(default)]
    pub section_title: String,
    #[serde(default)]
    pub section_code: String,

    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub logo_img_src: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub meeting_days: Vec<i8>,
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
    #[serde(default)]
    pub weight: String,
    #[serde(default)]
    pub materials: Folder,
}
