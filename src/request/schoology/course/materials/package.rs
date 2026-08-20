use super::{
    CourseMaterial, api_get, save,
    types::{integer, string},
};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, deserialize_with = "integer")]
    pub num_attempts: i64,
    #[serde(default, deserialize_with = "integer")]
    pub scorm_grading_enabled: i64,
    #[serde(default, deserialize_with = "integer")]
    pub sco_grading_enabled: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grade_timing_type: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grade_timing_option: i64,
    #[serde(default, deserialize_with = "integer")]
    pub available: i64,
    #[serde(default, deserialize_with = "integer")]
    pub completed: i64,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default, deserialize_with = "integer")]
    pub count_in_grade: i64,
}

/// Scrapes a SCORM package. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/scorm-package/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> PathBuf {
    info!("scraping Schoology SCORM package: {url}");
    let response: Package = api_get(url);
    save(material, &response, destination)
}
