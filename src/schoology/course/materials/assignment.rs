use super::{
    CourseMaterial,
    types::{ApiLinks, Attachments, LooseFloat, LooseInt},
};
use crate::{
    schoology::{RequestResult, api_get_with_query, types::datetime::SchoologyDatetime},
    types::LooseString,
};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Assignment {
    pub id: LooseString,
    pub title: String,
    pub description: String,
    pub due: SchoologyDatetime,
    pub grading_scale: LooseInt,
    pub grading_period: LooseInt,
    pub grading_category: LooseInt,
    pub max_points: LooseFloat,
    pub factor: LooseFloat,
    pub is_final: LooseInt,
    pub show_comments: LooseInt,
    pub grade_stats: LooseInt,
    pub allow_dropbox: LooseInt,
    pub allow_discussion: LooseInt,
    pub published: LooseInt,
    pub show_rubric: bool,
    pub assignees: Vec<i64>,
    pub grading_group_ids: Vec<i64>,
    pub count_in_grade: LooseInt,
    pub collected_only: LooseInt,
    pub auto_publish_grades: LooseInt,
    pub links: ApiLinks,
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
        course_id: String::new(), // filled later
        id: response.id.0,
        title: response.title,
        description: response.description,
        due: response.due.0,
        max_points: response.max_points.0,
        score: None, // populated by scrape_grades
        letter_grade: None,
        manual_mark: None,
        allow_submissions: response.allow_dropbox.0 != 0,
        attachments: response.attachments.into(),
        submissions: Vec::new(), // populated by scrape_submissions
    })
}
