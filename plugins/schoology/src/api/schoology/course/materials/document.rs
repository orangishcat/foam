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
pub struct Document {
    pub id: LooseString,
    pub title: String,
    pub url: String,
    pub body: String,
    pub course_fid: LooseInt,
    pub available: LooseInt,
    pub published: LooseInt,
    pub attachments: Attachments,
    pub display_inline: LooseInt,
    pub count_in_grade: LooseInt,
    pub collected_only: LooseInt,
    pub auto_publish_grades: LooseInt,
}

impl From<Document> for foam::types::document::Document {
    fn from(response: Document) -> Self {
        // Schoology documents that only embed web links/videos carry them as
        // link or video attachments and leave `url` empty; expose the first
        // embedded URL so the document view can open it.
        let url = if response.url.is_empty() {
            crate::api::schoology::course::materials::types::embedded_url(&response.attachments)
                .unwrap_or_default()
        } else {
            response.url
        };
        Self {
            id: response.id.0,
            title: response.title,
            url,
            body: response.body,
            attachments: response.attachments.into(),
        }
    }
}

impl SchoologyAccountConfig {
    /// Scrapes a document. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
    pub fn scrape_document(
        &self,
        _material: &CourseMaterial,
        url: &str,
    ) -> RequestResult<foam::types::document::Document> {
        info!("scraping Schoology document: {url}");
        let response: Document = self.api_get(url)?;
        // Schoology-specific payload (course_fid, availability flags, …);
        // the generic document view reads only the core fields.
        crate::api::schoology::plugin_data::store_material(
            "document",
            &response.id.0,
            &serde_json::to_value(&response)?,
        )?;
        Ok(response.into())
    }
}
