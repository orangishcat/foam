use std::{collections::HashSet, fs, path::PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::api_get;
use crate::config::config;

const API_ROOT: &str = "https://api.schoology.com/v1/courses";

#[derive(Debug, Clone)]
pub struct CourseFolderResponse {
    pub self_folder: CourseMaterial,
    pub parent: Option<CourseMaterial>,
    pub folder_items: Vec<CourseMaterial>,
}

#[derive(Debug, Clone)]
pub struct CourseMaterial {
    pub id: String,
    pub title: String,
    pub body: String,
    pub material_type: Option<String>,
    pub location: Option<String>,
    /// The material object exactly as returned by Schoology.
    pub raw: Value,
}

#[derive(Deserialize)]
struct RawFolderResponse {
    #[serde(rename = "self")]
    self_folder: Value,
    #[serde(default)]
    parent: Option<Value>,
    #[serde(default, rename = "folder-item")]
    folder_items: Vec<Value>,
}

/// Scrape every material below `folder_id`, preserving Schoology's folder tree.
///
/// Each raw material object is stored at
/// `courses/<course_id>/<containing_folder_id>/<material_id>.json`. The typed
/// response for the requested folder is returned for use by application code.
pub fn course(course_id: &str, folder_id: &str) -> CourseFolderResponse {
    let mut visited = HashSet::new();
    scrape_folder(course_id, folder_id, &mut visited)
        .expect("the requested Schoology course folder was visited more than once")
}

fn scrape_folder(
    course_id: &str,
    folder_id: &str,
    visited: &mut HashSet<String>,
) -> Option<CourseFolderResponse> {
    if !visited.insert(folder_id.to_owned()) {
        return None;
    }

    let url = format!("{API_ROOT}/{course_id}/folder/{folder_id}");
    let raw_response: Value = api_get(&url);
    let response: RawFolderResponse = serde_json::from_value(raw_response)
        .expect("failed to decode Schoology course folder response");
    let folder_dir = course_folder(course_id, folder_id);
    fs::create_dir_all(&folder_dir).expect("failed to create course folder data directory");

    let folder_items: Vec<CourseMaterial> = response
        .folder_items
        .into_iter()
        .map(CourseMaterial::from_raw)
        .collect();

    for material in &folder_items {
        let contents = serde_json::to_string_pretty(&material.raw)
            .expect("failed to serialize raw course material");
        fs::write(
            folder_dir.join(format!("{}.json", material.id)),
            format!("{contents}\n"),
        )
        .expect("failed to save course material");

        if material.material_type.as_deref() == Some("folder") {
            scrape_folder(course_id, &material.id, visited);
        }
    }

    Some(CourseFolderResponse {
        self_folder: CourseMaterial::from_raw(response.self_folder),
        parent: response.parent.map(CourseMaterial::from_raw),
        folder_items,
    })
}

impl CourseMaterial {
    fn from_raw(raw: Value) -> Self {
        let id = scalar_as_string(
            raw.get("id")
                .expect("Schoology course material is missing an id"),
        )
        .expect("Schoology course material id is not a string or number");

        Self {
            id,
            title: string_field(&raw, "title"),
            body: string_field(&raw, "body"),
            material_type: raw.get("type").and_then(Value::as_str).map(str::to_owned),
            location: raw
                .get("location")
                .and_then(Value::as_str)
                .map(str::to_owned),
            raw,
        }
    }
}

fn course_folder(course_id: &str, folder_id: &str) -> PathBuf {
    config()
        .data_dir()
        .join("courses")
        .join(course_id)
        .join(folder_id)
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
