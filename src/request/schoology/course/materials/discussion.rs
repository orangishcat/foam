use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, integer, string},
};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discussion {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default, deserialize_with = "integer")]
    pub uid: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default, deserialize_with = "integer")]
    pub weight: i64,
    #[serde(default, deserialize_with = "integer")]
    pub graded: i64,
    #[serde(default)]
    pub due: String,
    #[serde(default, deserialize_with = "integer")]
    pub grade_item_id: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grading_scale: i64,
    #[serde(default, deserialize_with = "integer")]
    pub published: i64,
    #[serde(default, deserialize_with = "integer")]
    pub available: i64,
    #[serde(default, deserialize_with = "integer")]
    pub completed: i64,
    #[serde(default, deserialize_with = "integer")]
    pub count_in_grade: i64,
    #[serde(default, deserialize_with = "integer")]
    pub collected_only: i64,
    #[serde(default, deserialize_with = "integer")]
    pub auto_publish_grades: i64,
    #[serde(default, deserialize_with = "integer")]
    pub comments_closed: i64,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub links: ApiLinks,
}

/// Scrapes a discussion. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/discussion-thread/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> PathBuf {
    info!("scraping Schoology discussion: {url}");
    let response: Discussion = api_get(url);
    save(material, &response, destination)
}
