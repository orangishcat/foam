use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::types::LooseString;

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct LooseInt(pub i64);

impl<'de> Deserialize<'de> for LooseInt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Number(v) => v
                .as_i64()
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom("integer is outside i64 range")),
            Value::String(v) if v.is_empty() => Ok(Self::default()),
            Value::String(v) => v.parse().map(Self).map_err(serde::de::Error::custom),
            Value::Bool(v) => Ok(Self(i64::from(v))),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected an integer, got {value}"
            ))),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct LooseFloat(pub f64);

impl<'de> Deserialize<'de> for LooseFloat {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Number(v) => v
                .as_f64()
                .map(Self)
                .ok_or_else(|| serde::de::Error::custom("number is outside f64 range")),
            Value::String(v) if v.is_empty() => Ok(Self::default()),
            Value::String(v) => v.parse().map(Self).map_err(serde::de::Error::custom),
            Value::Null => Ok(Self::default()),
            value => Err(serde::de::Error::custom(format!(
                "expected a number, got {value}"
            ))),
        }
    }
}

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
