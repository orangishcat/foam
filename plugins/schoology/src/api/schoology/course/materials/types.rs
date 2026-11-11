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
    /// Schoology embeds plain web links as link attachments; a document with
    /// only links has no `url` of its own.
    pub links: AttachmentLinks,
    /// Remaining attachment kinds (videos, media albums, …); scanned for
    /// openable URLs by `embedded_url`.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttachmentFiles {
    pub file: Vec<FileAttachment>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttachmentLinks {
    pub link: Vec<LinkAttachment>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkAttachment {
    pub id: LooseString,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub url: String,
    pub title: String,
    pub summary: String,
    pub display_inline: LooseInt,
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
    pub converted_download_path: String,
    pub converted_extension: String,
    pub converted_filesize: LooseInt,
}

impl From<Attachments> for foam::types::attachment::Attachments {
    fn from(value: Attachments) -> Self {
        Self {
            files: foam::types::attachment::AttachmentFiles {
                file: value.files.file.into_iter().map(Into::into).collect(),
            },
        }
    }
}

/// The first openable URL inside an attachments payload: explicit link
/// attachments first, then any remaining kind (videos, media, …) whose
/// entries carry a `url`.
pub fn embedded_url(value: &Attachments) -> Option<String> {
    if let Some(url) = value.links.link.iter().map(|link| link.url.as_str()).find(|url| !url.is_empty()) {
        return Some(url.to_owned());
    }
    for (_, group) in &value.extra {
        let Some(object) = group.as_object() else { continue };
        for (_, items) in object {
            let Some(items) = items.as_array() else { continue };
            for item in items {
                if let Some(url) = item.get("url").and_then(|url| url.as_str())
                    && !url.is_empty()
                {
                    return Some(url.to_owned());
                }
            }
        }
    }
    None
}

impl From<FileAttachment> for foam::types::attachment::FileAttachment {
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
            preview_url: value.converted_download_path,
            preview_extension: value.converted_extension,
            preview_filesize: value.converted_filesize.0,
        }
    }
}
