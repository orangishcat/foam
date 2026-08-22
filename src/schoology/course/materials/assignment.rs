use super::{
    CourseMaterial, api_get,
    types::{ApiLinks, LooseFloat, LooseInt, string},
};
use crate::schoology::RequestResult;
use log::info;
use serde::{Deserialize, Serialize};

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
pub fn scrape(
    _material: &CourseMaterial,
    url: &str,
) -> RequestResult<crate::types::assignment::Assignment> {
    info!("scraping Schoology assignment: {url}");
    let response: Assignment = api_get(url)?;
    Ok(crate::types::assignment::Assignment {
        id: response.id,
        title: response.title,
        description: response.description,
        due: response.due.parse().unwrap_or_default(),
        max_points: response.max_points.0,
        allow_submissions: response.allow_dropbox.0 != 0,
        show_rubric: response.show_rubric,
        assignees: response.assignees,
        grading_group_ids: response.grading_group_ids,
        count_in_grade: response.count_in_grade.0 != 0,
        collected_only: response.collected_only.0 != 0,
        auto_publish_grades: response.auto_publish_grades.0 != 0,
    })
}
