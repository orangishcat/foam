use serde::{Deserialize, Deserializer, Serialize};

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
    #[serde(default, deserialize_with = "integer")]
    pub filesize: i64,
    #[serde(default)]
    pub md5_checksum: String,
    #[serde(default, deserialize_with = "integer")]
    pub timestamp: i64,
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

pub fn integer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Integer(i64),
        String(String),
        Bool(bool),
    }
    match Option::<Repr>::deserialize(deserializer)? {
        Some(Repr::Integer(v)) => Ok(v),
        Some(Repr::String(v)) if v.is_empty() => Ok(0),
        Some(Repr::String(v)) => v.parse().map_err(serde::de::Error::custom),
        Some(Repr::Bool(v)) => Ok(i64::from(v)),
        None => Ok(0),
    }
}

pub fn float<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Float(f64),
        String(String),
    }
    match Option::<Repr>::deserialize(deserializer)? {
        Some(Repr::Float(v)) => Ok(v),
        Some(Repr::String(v)) if v.is_empty() => Ok(0.0),
        Some(Repr::String(v)) => v.parse().map_err(serde::de::Error::custom),
        None => Ok(0.0),
    }
}
