use super::{RequestResult, api_get};
use crate::types::{folder::Folder, material::Material};
use log::info;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashSet, io};

pub mod courses;
pub mod materials;
const API_ROOT: &str = "https://api.schoology.com/v1/courses";

#[derive(Debug, Clone)]
pub struct CourseMaterial {
    pub id: String,
    pub title: String,
    pub body: String,
    pub material_type: String,
    pub location: Option<String>,
}

#[derive(Deserialize)]
struct RawFolderResponse {
    #[serde(rename = "self")]
    self_folder: Value,
    #[serde(default, rename = "folder-item")]
    folder_items: Vec<Value>,
}

/// Fetch a complete Schoology material tree as a standardized folder.
pub fn course(course_id: &str, folder_id: &str) -> RequestResult<Folder> {
    info!("scraping Schoology course tree: course={course_id}, folder={folder_id}");
    scrape_folder(course_id, folder_id, None, &mut HashSet::new())
}

fn scrape_folder(
    course_id: &str,
    folder_id: &str,
    url: Option<&str>,
    visited: &mut HashSet<String>,
) -> RequestResult<Folder> {
    if !visited.insert(folder_id.to_owned()) {
        return Err(io::Error::other(format!("course folder cycle at {folder_id}")).into());
    }
    let fallback_url = format!("{API_ROOT}/{course_id}/folder/{folder_id}");
    let raw: RawFolderResponse = api_get(url.unwrap_or(&fallback_url))?;
    let meta = CourseMaterial::from_raw(raw.self_folder)?;
    let mut folder = Folder {
        id: meta.id,
        title: meta.title,
        body: meta.body,
        materials: Vec::with_capacity(raw.folder_items.len()),
    };
    for item in raw.folder_items {
        let material = CourseMaterial::from_raw(item)?;
        if material.material_type == "folder" {
            let child = scrape_folder(
                course_id,
                &material.id,
                material.location.as_deref(),
                visited,
            )?;
            folder.materials.push(Material::Folder(Box::new(child)));
        } else {
            if let Some(material) = materials::scrape(&material)? {
                folder.materials.push(material);
            }
        }
    }
    Ok(folder)
}

impl CourseMaterial {
    fn from_raw(raw: Value) -> RequestResult<Self> {
        let id = raw
            .get("id")
            .and_then(scalar_as_string)
            .ok_or_else(|| io::Error::other("Schoology course material is missing an id"))?;
        Ok(Self {
            id,
            title: field(&raw, "title"),
            body: field(&raw, "body"),
            material_type: field(&raw, "type"),
            location: raw
                .get("location")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

fn field(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.clone()),
        Value::Number(v) => Some(v.to_string()),
        _ => None,
    }
}
