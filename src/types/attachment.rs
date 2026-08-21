use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Attachments {
    #[serde(default)]
    pub files: AttachmentFiles,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttachmentFiles {
    #[serde(default)]
    pub file: Vec<FileAttachment>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileAttachment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub attachment_type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub filesize: i64,
    #[serde(default)]
    pub md5_checksum: String,
    #[serde(default)]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub filemime: String,
    #[serde(default)]
    pub download_path: String,
    #[serde(default)]
    pub extension: String,
}
