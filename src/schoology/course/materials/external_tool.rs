use super::{CourseMaterial, api_get, save, types::string};
use crate::{schoology::RequestResult, types::LooseInt};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub count_in_grade: LooseInt,
    #[serde(default)]
    pub collected_only: LooseInt,
    #[serde(default)]
    pub auto_publish_grades: LooseInt,
}

/// Scrapes an external tool. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology external tool: {url}");
    let response: ExternalTool = api_get(url)?;
    save(material, &response, destination)
}
