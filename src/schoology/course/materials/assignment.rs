use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, string},
};
use crate::{
    schoology::RequestResult,
    types::{LooseFloat, LooseInt},
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
    #[serde(default)]
    pub grading_scale: LooseInt,
    #[serde(default)]
    pub grading_period: LooseInt,
    #[serde(default)]
    pub grading_category: LooseInt,
    #[serde(default)]
    pub max_points: LooseFloat,
    #[serde(default)]
    pub factor: LooseFloat,
    #[serde(default)]
    pub is_final: LooseInt,
    #[serde(default)]
    pub show_comments: LooseInt,
    #[serde(default)]
    pub grade_stats: LooseInt,
    #[serde(default)]
    pub allow_dropbox: LooseInt,
    #[serde(default)]
    pub allow_discussion: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub show_rubric: bool,
    #[serde(default)]
    pub assignees: Vec<i64>,
    #[serde(default)]
    pub grading_group_ids: Vec<i64>,
    #[serde(default)]
    pub count_in_grade: LooseInt,
    #[serde(default)]
    pub collected_only: LooseInt,
    #[serde(default)]
    pub auto_publish_grades: LooseInt,
    #[serde(default)]
    pub links: ApiLinks,
}

/// Scrapes an assignment. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology assignment: {url}");
    let response: Assignment = api_get(url)?;
    save(material, &response, destination)
}
