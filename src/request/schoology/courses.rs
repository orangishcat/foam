use std::fs;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::{api_get, internal_get};
use crate::config::CONFIG;

const INTERNAL_ROUTE: &str = "/iapi2/site-navigation/courses";
const API_ROUTE: &str = "https://api.schoology.com/v1/users/me/sections";
const DEFAULT_LOGO: &str =
    "https://asset-cdn.schoology.com/sites/all/themes/schoology_theme/images/course-default.svg";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoursesResponse {
    pub data: CoursesData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoursesData {
    pub courses: Vec<Course>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub nid: u64,
    pub course_title: String,
    pub section_title: String,
    pub building_title: String,
    pub logo_img_src: String,
    pub course_nid: u64,
    pub weight: i64,
    pub course_landing_page_type: String,
    pub is_csl: bool,
    pub admin_type: String,
}

#[derive(Deserialize)]
struct ApiSectionsResponse {
    section: Vec<ApiSection>,
}

#[derive(Deserialize)]
struct ApiSection {
    id: String,
    course_id: String,
    course_title: String,
    section_title: String,
    profile_url: String,
    weight: String,
    admin: i64,
}

pub fn courses() -> CoursesResponse {
    let use_internal = {
        let config = CONFIG.read().expect("config lock is poisoned");
        !config.cookie_key.is_empty()
    };

    let response = if use_internal {
        internal_get(INTERNAL_ROUTE)
    } else {
        let api_response: ApiSectionsResponse = api_get(API_ROUTE);
        CoursesResponse {
            data: CoursesData {
                courses: api_response
                    .section
                    .into_iter()
                    .map(|section| Course {
                        nid: section
                            .id
                            .parse()
                            .expect("Schoology returned an invalid section ID"),
                        course_title: section.course_title,
                        section_title: section.section_title,
                        building_title: String::new(),
                        logo_img_src: if section.profile_url.is_empty() {
                            DEFAULT_LOGO.to_string()
                        } else {
                            section.profile_url
                        },
                        course_nid: section
                            .course_id
                            .parse()
                            .expect("Schoology returned an invalid course ID"),
                        weight: section
                            .weight
                            .parse()
                            .expect("Schoology returned an invalid course weight"),
                        course_landing_page_type: "materials".to_string(),
                        is_csl: false,
                        admin_type: if section.admin == 0 { "none" } else { "admin" }.to_string(),
                    })
                    .collect(),
            },
        }
    };

    let mut config = CONFIG.write().expect("config lock is poisoned");
    for course in &response.data.courses {
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
