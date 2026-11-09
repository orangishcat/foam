use super::{
    CourseMaterial,
    types::{ApiLinks, Attachments},
};
use crate::api::schoology::account::SchoologyAccountConfig;
use crate::api::{
    schoology::{RequestResult, types::datetime::SchoologyDatetime},
    types::{LooseFloat, LooseInt, LooseString},
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SchoologyAssignment {
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

impl SchoologyAccountConfig {
    /// Scrapes an assignment. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/assignment/>
    pub fn scrape_assignment(
        &self,
        _material: &CourseMaterial,
        url: &str,
    ) -> RequestResult<foam::types::task::Task> {
        log::debug!("scraping Schoology assignment: {url}");
        let query_params = AttachmentQuery {
            with_attachments: true,
        };
        let response: SchoologyAssignment = self.api_get_with_query(url, &query_params)?;
        Ok(foam::types::task::Task {
            folder_id: String::new(), // filled later
            id: response.id.0,
            source: "builtin".to_owned(),
            title: response.title,
            description: response.description,
            due: response.due.0,
            attachments: response.attachments.into(),
        })
    }
}
