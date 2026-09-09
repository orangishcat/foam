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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Assessment {
    pub id: LooseString,
    pub title: String,
    pub description: String,
    pub max_points: LooseFloat,
    pub due: SchoologyDatetime,
    pub grading_scale: LooseInt,
    pub grading_period: LooseInt,
    pub published: LooseInt,
    pub available: LooseInt,
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
