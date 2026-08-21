use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, string},
};
use crate::{schoology::RequestResult, types::LooseInt};
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
    #[serde(default)]
    pub parent: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub inline: LooseInt,
    #[serde(default)]
    pub created: LooseInt,
    #[serde(default)]
    pub children: Vec<i64>,
    #[serde(default)]
    pub num_assignees: LooseInt,
    #[serde(default)]
    pub assignees: Vec<i64>,
    #[serde(default)]
    pub grading_group_ids: Vec<i64>,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
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
