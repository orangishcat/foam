use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, integer, string},
};
use crate::request::schoology::RequestResult;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default, deserialize_with = "integer")]
    pub parent: i64,
    #[serde(default, deserialize_with = "integer")]
    pub published: i64,
    #[serde(default, deserialize_with = "integer")]
    pub inline: i64,
    #[serde(default, deserialize_with = "integer")]
    pub created: i64,
    #[serde(default)]
    pub children: Vec<i64>,
    #[serde(default, deserialize_with = "integer")]
    pub num_assignees: i64,
    #[serde(default)]
    pub assignees: Vec<i64>,
    #[serde(default)]
    pub grading_group_ids: Vec<i64>,
    #[serde(default, deserialize_with = "integer")]
    pub available: i64,
    #[serde(default, deserialize_with = "integer")]
    pub completed: i64,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub links: ApiLinks,
}

/// Scrapes a page. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/pages/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology page: {url}");
    let response: Page = api_get(url)?;
    save(material, &response, destination)
}
