use crate::types::attachment::Attachments;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    pub id: String,
    pub title: String,
    pub url: String,
    pub attachments: Attachments,
}
