use super::CourseMaterial;
use crate::api::schoology::account::SchoologyAccountConfig;
use crate::api::{
    schoology::{RequestResult, types::datetime::SchoologyDatetime},
    types::{LooseFloat, LooseInt, LooseString},
};
use log::info;
use serde::{Deserialize, Serialize};

/// Schoology wire type for an assessment/test-quiz. It never lands in core
/// tables: the payload is stored in `plugin_schoology_materials`, and the
/// core resource row keeps only the generic label.
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

impl SchoologyAccountConfig {
    /// Scrapes an assessment or test/quiz. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
    pub fn scrape_assessment(
        &self,
        _material: &CourseMaterial,
        url: &str,
    ) -> RequestResult<Assessment> {
        info!("scraping Schoology assessment: {url}");
        let response: Assessment = self.api_get(url)?;
        crate::api::schoology::plugin_data::store_material(
            "assessment",
            &response.id.0,
            &serde_json::to_value(&response)?,
        )?;
        Ok(response)
    }
}
