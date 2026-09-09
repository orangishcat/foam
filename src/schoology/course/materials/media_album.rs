use crate::types::LooseString;

use super::types::{ApiLinks, LooseInt};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaAlbum {
    pub id: LooseString,
    pub title: String,
    pub description: String,
    pub setting_comments: LooseInt,
    pub setting_member_post: LooseInt,
    pub published: LooseInt,
    pub photo_count: LooseInt,
    pub video_count: LooseInt,
    pub audio_count: LooseInt,
    pub cover_image_url: String,
    pub created: LooseInt,
    pub available: LooseInt,
    pub completed: LooseInt,
    pub completion_status: String,
    pub links: ApiLinks,
}
