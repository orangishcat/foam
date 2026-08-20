use std::fs;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::api_get;
use crate::config::CONFIG;

const API_ROUTE: &str = "https://api.schoology.com/v1/users/me/sections";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoursesResponse {
    pub section: Vec<Course>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    #[serde(rename = "id")]
    pub nid: String,
    pub course_title: String,
    pub section_title: String,
    #[serde(rename = "profile_url")]
    pub logo_img_src: String,
}

pub fn courses() -> CoursesResponse {
    let response: CoursesResponse = api_get(API_ROUTE);

    let mut config = CONFIG.write().expect("config lock is poisoned");
    for course in &response.section {
        let course_dir = config
            .data_dir()
            .join("courses")
            .join(course.nid.to_string());
        fs::create_dir_all(&course_dir).expect("failed to create course data directory");
        let contents = serde_json::to_string_pretty(course).expect("failed to serialize course");
        fs::write(course_dir.join("course.json"), format!("{contents}\n"))
            .expect("failed to save course");
    }

    config.last_updated = Some(Utc::now());
    config.save().expect("failed to save configuration");
    response
}
