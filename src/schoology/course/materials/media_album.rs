use super::{
    CourseMaterial, api_get, save,
    types::{ApiLinks, integer, string},
};
use crate::schoology::RequestResult;
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
    #[serde(default, deserialize_with = "integer")]
    pub setting_comments: i64,
    #[serde(default, deserialize_with = "integer")]
    pub setting_member_post: i64,
    #[serde(default, deserialize_with = "integer")]
    pub published: i64,
    #[serde(default, deserialize_with = "integer")]
    pub photo_count: i64,
    #[serde(default, deserialize_with = "integer")]
    pub video_count: i64,
    #[serde(default, deserialize_with = "integer")]
    pub audio_count: i64,
    #[serde(default)]
    pub cover_image_url: String,
    #[serde(default, deserialize_with = "integer")]
    pub created: i64,
    #[serde(default, deserialize_with = "integer")]
    pub available: i64,
    #[serde(default, deserialize_with = "integer")]
    pub completed: i64,
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
