use super::{
    CourseMaterial,
    types::{ApiLinks, Attachments, LooseFloat, LooseInt},
};
use crate::{
    schoology::{RequestResult, api_get_with_query},
    types::LooseString,
};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    #[serde(default)]
    pub id: LooseString,
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
    #[serde(default)]
    pub attachments: Attachments,
}

#[derive(Serialize, oauth::Request)]
struct AttachmentQuery {
    with_attachments: bool,
}

/// Scrapes an assignment. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
pub fn scrape(
    _material: &CourseMaterial,
    url: &str,
) -> RequestResult<crate::types::assignment::Assignment> {
    info!("scraping Schoology assignment: {url}");
    let query_params = AttachmentQuery {
        with_attachments: true,
    };
    let response: Assignment = api_get_with_query(url, &query_params)?;
    Ok(crate::types::assignment::Assignment {
        id: response.id.0,
        title: response.title,
        description: response.description,
        due: response.due.parse().unwrap_or_default(),
        max_points: response.max_points.0,
        allow_submissions: response.allow_dropbox.0 != 0,
        attachments: response.attachments.into(),
    })
}
