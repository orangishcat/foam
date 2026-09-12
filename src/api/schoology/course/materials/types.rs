use serde::{Deserialize, Serialize};

use crate::api::types::{LooseInt, LooseString};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiLinks {
    #[serde(rename = "self")]
    pub self_url: String,
}

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
    pub id: LooseString,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub title: String,
    pub filename: String,
    pub filesize: LooseInt,
    pub md5_checksum: String,
    pub timestamp: LooseInt,
    pub filemime: String,
    pub download_path: String,
    pub extension: String,
}

impl From<Attachments> for crate::types::attachment::Attachments {
    fn from(value: Attachments) -> Self {
        Self {
            files: crate::types::attachment::AttachmentFiles {
                file: value.files.file.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<FileAttachment> for crate::types::attachment::FileAttachment {
    fn from(value: FileAttachment) -> Self {
        Self {
            id: value.id.0,
            attachment_type: value.attachment_type,
            title: value.title,
            filename: value.filename,
            filesize: value.filesize.0,
            md5_checksum: value.md5_checksum,
            timestamp: chrono::DateTime::from_timestamp(value.timestamp.0, 0).unwrap_or_default(),
            filemime: value.filemime,
            download_path: value.download_path,
            extension: value.extension,
        }
    }
}
