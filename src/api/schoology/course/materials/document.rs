use super::{CourseMaterial, types::Attachments};
use crate::api::schoology::account::SchoologyAccountConfig;
use crate::api::{
    schoology::RequestResult,
    types::{LooseInt, LooseString},
};
use crate::types::material::Material;
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

impl From<Document> for crate::types::document::Document {
    fn from(response: Document) -> Self {
        Self {
            id: response.id.0,
            title: response.title,
            url: response.url,
            course_fid: response.course_fid.0,
            attachments: response.attachments.into(),
        }
    }
}

impl From<Document> for crate::types::link::Link {
    fn from(response: Document) -> Self {
        Self {
            id: response.id.0,
            title: response.title,
            url: response.url,
            course_fid: response.course_fid.0,
            available: response.available.0 != 0,
            published: response.published.0 != 0,
            attachments: response.attachments.into(),
            display_inline: response.display_inline.0 != 0,
        }
    }
}

impl SchoologyAccountConfig {
    /// Scrapes a document. Schoology documents are either files or links; a link has a `url` and no file attachments.
    /// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/documents/>
    pub fn scrape_document(
        &self,
        _material: &CourseMaterial,
        url: &str,
    ) -> RequestResult<Material> {
        info!("scraping Schoology document: {url}");
        let response: Document = self.api_get(url)?;
        if response.attachments.files.file.is_empty() && !response.url.trim().is_empty() {
            return Ok(Material::Link(response.into()));
        }
        Ok(Material::Document(response.into()))
    }
}
