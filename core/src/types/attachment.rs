use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachments {
    pub files: AttachmentFiles,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttachmentFiles {
    pub file: Vec<FileAttachment>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FileAttachment {
    pub id: String,
    pub attachment_type: String,
    pub title: String,
    pub filename: String,
    pub filesize: i64,
    pub md5_checksum: String,
    pub timestamp: DateTime<Utc>,
    pub filemime: String,
    pub download_path: String,
    pub extension: String,
}
