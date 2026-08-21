use super::{CourseMaterial, api_get, save, types::string};
use crate::{
    schoology::RequestResult,
    types::{LooseFloat, LooseInt},
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
    #[serde(default)]
    pub max_points: LooseFloat,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub grading_scale: LooseInt,
    #[serde(default)]
    pub grading_period: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
}

/// Scrapes an assessment or test/quiz. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology assessment: {url}");
    let response: Assessment = api_get(url)?;
    save(material, &response, destination)
}
