use super::{
    CourseMaterial, api_get, save,
    types::{Attachments, integer, string},
};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, deserialize_with = "integer")]
    pub course_fid: i64,
    #[serde(default, deserialize_with = "integer")]
    pub available: i64,
    #[serde(default, deserialize_with = "integer")]
    pub published: i64,
    #[serde(default)]
    pub attachments: Attachments,
    #[serde(default, deserialize_with = "integer")]
    pub display_inline: i64,
    #[serde(default, deserialize_with = "integer")]
    pub count_in_grade: i64,
    #[serde(default, deserialize_with = "integer")]
    pub collected_only: i64,
    #[serde(default, deserialize_with = "integer")]
    pub auto_publish_grades: i64,
}

/// Scrapes a document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> PathBuf {
    info!("scraping Schoology document: {url}");
    let response: Document = api_get(url);
    save(material, &response, destination)
}
