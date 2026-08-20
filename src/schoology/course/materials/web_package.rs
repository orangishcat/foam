use super::{
    CourseMaterial, api_get, save,
    types::{integer, string},
};
use crate::schoology::RequestResult;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPackage {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, deserialize_with = "integer")]
    pub uid: i64,
    #[serde(default)]
    pub url: String,
}

/// Scrapes a web content package. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/web-content-package/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology web content package: {url}");
    let response: WebPackage = api_get(url)?;
    save(material, &response, destination)
}
