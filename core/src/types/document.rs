use crate::types::attachment::Attachments;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub url: String,
    /// Inline HTML body of Schoology page-style documents; empty for file
    /// documents, which carry the content as an attachment download.
    pub body: String,
    pub attachments: Attachments,
}
