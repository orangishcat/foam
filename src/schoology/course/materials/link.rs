use super::{
    CourseMaterial, api_get, save,
    types::{Attachments, string},
};
use crate::{schoology::RequestResult, types::LooseInt};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub course_fid: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub attachments: Attachments,
    #[serde(default)]
    pub display_inline: LooseInt,
}

/// Scrapes a link document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology link: {url}");
    let response: Link = api_get(url)?;
    save(material, &response, destination)
}
