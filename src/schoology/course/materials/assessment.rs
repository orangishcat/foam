use super::{
    CourseMaterial, api_get,
    types::{LooseFloat, LooseInt, string},
};
use crate::schoology::RequestResult;
use chrono::Utc;
use log::info;
use serde::{Deserialize, Serialize};

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
pub fn scrape(
    _material: &CourseMaterial,
    url: &str,
) -> RequestResult<crate::types::assessment::Assessment> {
    info!("scraping Schoology assessment: {url}");
    let response: Assessment = api_get(url)?;
    Ok(crate::types::assessment::Assessment {
        id: response.id,
        title: response.title,
        description: response.description,
        max_points: response.max_points.0,
        due: chrono::DateTime::parse_from_str(&response.due, "%Y-%M-%D %H:%M:%s")
            .unwrap_or_default()
            .with_timezone(&Utc),
        completed: response.completed.0 != 0,
    })
}
