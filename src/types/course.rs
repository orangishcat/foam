use serde::{Deserialize, Serialize};
use serde_with::{PickFirst, Same, json::JsonString, serde_as};

use super::folder::Folder;

#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Course {
    pub course_id: String, // in schoology this is actually the section id
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub aliases: Vec<String>,
    pub course_title: String,
    pub course_code: String,
    pub course_url: String,
    pub section_title: String,
    pub section_code: String,

    pub active: bool,
    pub description: String,
    pub logo_img_src: String,
    pub location: String,
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub meeting_days: Vec<i8>,
    pub start_time: String,
    pub end_time: String,
    pub weight: String,
}
