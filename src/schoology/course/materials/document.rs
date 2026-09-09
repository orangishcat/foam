use super::{
    CourseMaterial, api_get,
    types::{Attachments, LooseInt},
};
use crate::{schoology::RequestResult, types::LooseString};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub id: LooseString,
    pub title: String,
    pub url: String,
    pub course_fid: LooseInt,
    pub available: LooseInt,
    pub published: LooseInt,
    pub attachments: Attachments,
    pub display_inline: LooseInt,
    pub count_in_grade: LooseInt,
    pub collected_only: LooseInt,
    pub auto_publish_grades: LooseInt,
}

/// Scrapes a document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
pub fn scrape(
    _material: &CourseMaterial,
    url: &str,
) -> RequestResult<crate::types::document::Document> {
    info!("scraping Schoology document: {url}");
    let response: Document = api_get(url)?;
    Ok(crate::types::document::Document {
        id: response.id.0,
        title: response.title,
        url: response.url,
        course_fid: response.course_fid.0,
        attachments: response.attachments.into(),
    })
}
