use super::{
    CourseMaterial, api_get,
    types::{Attachments, LooseInt},
};
use crate::{schoology::RequestResult, types::LooseString};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    pub id: LooseString,
    pub title: String,
    pub url: String,
    pub course_fid: LooseInt,
    pub available: LooseInt,
    pub published: LooseInt,
    pub attachments: Attachments,
    pub display_inline: LooseInt,
}

/// Scrapes a link document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
pub fn scrape(_material: &CourseMaterial, url: &str) -> RequestResult<crate::types::link::Link> {
    info!("scraping Schoology link: {url}");
    let response: Link = api_get(url)?;
    Ok(crate::types::link::Link {
        id: response.id.0,
        title: response.title,
        url: response.url,
        course_fid: response.course_fid.0,
        available: response.available.0 != 0,
        published: response.published.0 != 0,
        attachments: response.attachments.into(),
        display_inline: response.display_inline.0 != 0,
    })
}
