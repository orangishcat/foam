use serde::{Deserialize, Deserializer, Serialize};

use crate::types::LooseInt;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiLinks {
    #[serde(default, rename = "self")]
    pub self_url: String,
}

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
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default, rename = "type")]
    pub attachment_type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub filesize: LooseInt,
    #[serde(default)]
    pub md5_checksum: String,
    #[serde(default)]
    pub timestamp: LooseInt,
    #[serde(default)]
    pub filemime: String,
    #[serde(default)]
    pub download_path: String,
    #[serde(default)]
    pub extension: String,
}

pub fn string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        String(String),
        Signed(i64),
        Unsigned(u64),
        Float(f64),
    }
    Ok(match Option::<Repr>::deserialize(deserializer)? {
        Some(Repr::String(v)) => v,
        Some(Repr::Signed(v)) => v.to_string(),
        Some(Repr::Unsigned(v)) => v.to_string(),
        Some(Repr::Float(v)) => v.to_string(),
        None => String::new(),
    })
}
