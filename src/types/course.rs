use serde::{Deserialize, Serialize};

use super::folder::Folder;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Course {
    pub course_id: String,
    pub course_title: String,
    pub course_code: String,
    pub course_url: String,
    pub section_title: String,
    pub section_code: String,

    pub active: bool,
    pub description: String,
    pub logo_img_src: String,
    pub location: String,
    pub meeting_days: Vec<i8>,
    pub start_time: String,
    pub end_time: String,
    pub weight: String,
    pub materials: Folder,
}
