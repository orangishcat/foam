use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, float, integer, string},
};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub due: String,
    #[serde(default, deserialize_with = "integer")]
    pub grading_scale: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grading_period: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grading_category: i64,
    #[serde(default, deserialize_with = "float")]
    pub max_points: f64,
    #[serde(default, deserialize_with = "float")]
    pub factor: f64,
    #[serde(default, deserialize_with = "integer")]
    pub is_final: i64,
    #[serde(default, deserialize_with = "integer")]
    pub show_comments: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grade_stats: i64,
    #[serde(default, deserialize_with = "integer")]
    pub allow_dropbox: i64,
    #[serde(default, deserialize_with = "integer")]
    pub allow_discussion: i64,
    #[serde(default, deserialize_with = "integer")]
    pub published: i64,
    #[serde(default)]
    pub show_rubric: bool,
    #[serde(default)]
    pub assignees: Vec<i64>,
    #[serde(default)]
    pub grading_group_ids: Vec<i64>,
    #[serde(default, deserialize_with = "integer")]
    pub count_in_grade: i64,
    #[serde(default, deserialize_with = "integer")]
    pub collected_only: i64,
    #[serde(default, deserialize_with = "integer")]
    pub auto_publish_grades: i64,
    #[serde(default)]
    pub links: ApiLinks,
}

/// Scrapes an assignment. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> PathBuf {
    info!("scraping Schoology assignment: {url}");
    let response: Assignment = api_get(url);
    save(material, &response, destination)
}
