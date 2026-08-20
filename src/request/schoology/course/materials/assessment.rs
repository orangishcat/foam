use super::{
    CourseMaterial, api_get, save,
    types::{float, integer, string},
};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "float")]
    pub max_points: f64,
    #[serde(default)]
    pub due: String,
    #[serde(default, deserialize_with = "integer")]
    pub grading_scale: i64,
    #[serde(default, deserialize_with = "integer")]
    pub grading_period: i64,
    #[serde(default, deserialize_with = "integer")]
    pub published: i64,
    #[serde(default, deserialize_with = "integer")]
    pub available: i64,
    #[serde(default, deserialize_with = "integer")]
    pub completed: i64,
}

/// Scrapes an assessment or test/quiz. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> PathBuf {
    info!("scraping Schoology assessment: {url}");
    let response: Assessment = api_get(url);
    save(material, &response, destination)
}
