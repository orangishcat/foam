use super::{
    CourseMaterial, api_get,
    types::{Attachments, LooseInt},
};
use crate::{schoology::RequestResult, types::LooseString};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub course_fid: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub attachments: Attachments,
    #[serde(default)]
    pub display_inline: LooseInt,
    #[serde(default)]
    pub count_in_grade: LooseInt,
    #[serde(default)]
    pub collected_only: LooseInt,
    #[serde(default)]
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
