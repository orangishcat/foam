use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, string},
};
use crate::{schoology::RequestResult, types::LooseInt};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAlbum {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub setting_comments: LooseInt,
    #[serde(default)]
    pub setting_member_post: LooseInt,
    #[serde(default)]
    pub published: LooseInt,
    #[serde(default)]
    pub photo_count: LooseInt,
    #[serde(default)]
    pub video_count: LooseInt,
    #[serde(default)]
    pub audio_count: LooseInt,
    #[serde(default)]
    pub cover_image_url: String,
    #[serde(default)]
    pub created: LooseInt,
    #[serde(default)]
    pub available: LooseInt,
    #[serde(default)]
    pub completed: LooseInt,
    #[serde(default)]
    pub completion_status: String,
    #[serde(default)]
    pub links: ApiLinks,
}

/// Scrapes a media album. Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/media-album/>
pub fn scrape(material: &CourseMaterial, url: &str, destination: &Path) -> RequestResult<PathBuf> {
    info!("scraping Schoology media album: {url}");
    let response: MediaAlbum = api_get(url)?;
    save(material, &response, destination)
}
