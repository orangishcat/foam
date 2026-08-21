use super::{CourseMaterial, api_get, save, types::string};
use crate::{schoology::RequestResult, types::LooseInt};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub num_attempts: LooseInt,
    #[serde(default)]
    pub scorm_grading_enabled: LooseInt,
    #[serde(default)]
    pub sco_grading_enabled: LooseInt,
    #[serde(default)]
    pub grade_timing_type: LooseInt,
    #[serde(default)]
    pub grade_timing_option: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub count_in_grade: LooseInt,
}

/// Scrapes a SCORM package. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/scorm-package/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology SCORM package: {url}");
    let response: Package = api_get(url)?;
    save(material, &response, destination)
}
