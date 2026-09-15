use super::{RequestResult, api_get};
use crate::types::{folder::Folder, material::Material};
use log::info;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io,
};

pub mod courses;
pub mod grades;
pub mod materials;
pub mod submissions;
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
    let mut folder = scrape_folder(course_id, folder_id, None, &mut HashSet::new(), None)?;
    folder.set_course_id(course_id);
    Ok(folder)
}

/// Refresh folder metadata and placement without fetching any material details.
pub fn hierarchy(course_id: &str, existing: &Folder) -> RequestResult<Folder> {
    let cached = existing
        .recursive_iter()
        .map(|m| {
            (
                (
                    crate::types::material::MaterialType::from(m),
                    material_id(m).to_owned(),
                ),
                m.clone(),
            )
        })
        .collect();
    scrape_folder(course_id, "0", None, &mut HashSet::new(), Some(&cached))
}

/// Find an assignment's parent from folder metadata, including uncached assignments.
pub fn assignment_parent(course_id: &str, assignment_id: &str) -> RequestResult<Option<String>> {
    let mut pending = vec![("0".to_owned(), None::<String>)];
    let mut visited = HashSet::new();
    while let Some((folder_id, url)) = pending.pop() {
        crate::thread_manager::check_cancelled()?;
        if !visited.insert(folder_id.clone()) {
            return Err(io::Error::other(format!("course folder cycle at {folder_id}")).into());
        }
        let fallback_url = format!("{API_ROOT}/{course_id}/folder/{folder_id}");
        let raw: RawFolderResponse = api_get(url.as_deref().unwrap_or(&fallback_url))?;
        for item in raw.folder_items {
            let material = CourseMaterial::from_raw(item)?;
            if material.material_type == "folder" {
                pending.push((material.id, material.location));
            } else if material.id == assignment_id
                && matches!(
                    material.material_type.as_str(),
                    "assignment" | "assessment" | "test/quiz" | "quiz"
                )
            {
                return Ok(Some(folder_id));
            }
        }
    }
    Ok(None)
}

pub(crate) fn material_id(material: &Material) -> &str {
    match material {
        Material::Folder(m) => &m.id,
        Material::Assignment(m) => &m.id,
        Material::Document(m) => &m.id,
        Material::Assessment(m) => &m.id,
        Material::Link(m) => &m.id,
    }
}

fn scrape_folder(
    course_id: &str,
    folder_id: &str,
    url: Option<&str>,
    visited: &mut HashSet<String>,
    cached: Option<&HashMap<(crate::types::material::MaterialType, String), Material>>,
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
                cached,
            )?;
            folder.materials.push(Material::Folder(Box::new(child)));
        } else if let Some(cached) = cached {
            use crate::types::material::MaterialType;
            let kind = match material.material_type.as_str() {
                "assignment" => Some(MaterialType::Assignment),
                "document" => Some(MaterialType::Document),
                "link" => Some(MaterialType::Link),
                "assessment" | "test/quiz" | "quiz" => Some(MaterialType::Assessment),
                _ => None,
            };
            if let Some(value) = kind.and_then(|kind| cached.get(&(kind, material.id))) {
                folder.materials.push(value.clone());
            }
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
