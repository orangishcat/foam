use super::{
    CourseMaterial, api_get,
    types::{Attachments, LooseInt, string},
};
use crate::schoology::RequestResult;
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
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
}

/// Scrapes a link document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
pub fn scrape(_material: &CourseMaterial, url: &str) -> RequestResult<crate::types::link::Link> {
    info!("scraping Schoology link: {url}");
    let response: Link = api_get(url)?;
    Ok(crate::types::link::Link {
        id: response.id,
        title: response.title,
        url: response.url,
        course_fid: response.course_fid.0,
        available: response.available.0 != 0,
        published: response.published.0 != 0,
        attachments: response.attachments.into(),
        display_inline: response.display_inline.0 != 0,
    })
}
