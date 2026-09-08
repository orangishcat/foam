use super::{
    CourseMaterial, api_get,
    types::{LooseFloat, LooseInt},
};
use crate::{
    schoology::{RequestResult, types::datetime::SchoologyDatetime},
    types::LooseString,
};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub max_points: LooseFloat,
    #[serde(default)]
    pub due: SchoologyDatetime,
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
pub fn scrape(
    _material: &CourseMaterial,
    url: &str,
) -> RequestResult<crate::types::assessment::Assessment> {
    info!("scraping Schoology assessment: {url}");
    let response: Assessment = api_get(url)?;
    Ok(crate::types::assessment::Assessment {
        id: response.id.0,
        title: response.title,
        description: response.description,
        max_points: response.max_points.0,
        due: response.due.0,
        completed: response.completed.0 != 0,
    })
}
