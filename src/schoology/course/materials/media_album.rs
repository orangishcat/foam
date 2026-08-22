use crate::types::LooseString;

use super::types::{ApiLinks, LooseInt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAlbum {
    #[serde(default)]
    pub id: LooseString,
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
