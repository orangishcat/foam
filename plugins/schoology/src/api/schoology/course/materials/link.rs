use super::{CourseMaterial, types::Attachments};
use crate::api::schoology::account::SchoologyAccountConfig;
use crate::api::{
    schoology::RequestResult,
    types::{LooseInt, LooseString},
};
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

impl SchoologyAccountConfig {
    /// Scrapes a link document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
    pub fn scrape_link(
        &self,
        _material: &CourseMaterial,
        url: &str,
    ) -> RequestResult<foam::types::link::Link> {
        info!("scraping Schoology link: {url}");
        let response: Link = self.api_get(url)?;
        // Schoology-specific payload stays on the plugin's own table; core
        // links keep only id/title/attachments/url.
        crate::api::schoology::plugin_data::store_material(
            "link",
            &response.id.0,
            &serde_json::to_value(&response)?,
        )?;
        Ok(foam::types::link::Link {
            id: response.id.0,
            title: response.title,
            url: response.url,
            attachments: response.attachments.into(),
        })
    }
}
