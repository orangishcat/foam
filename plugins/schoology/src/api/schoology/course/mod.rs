use super::RequestResult;
use crate::api::schoology::account::SchoologyAccountConfig;
use foam::types::folder::Folder;
use foam::types::resource::{Resource, ResourceType};
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

/// A Schoology course-material listing entry (API wire type).
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

pub(crate) fn resource_id(resource: &Resource) -> &str {
    match resource {
        Resource::Folder(m) => &m.id,
        Resource::Task(m) => &m.id,
        Resource::Document(m) => &m.id,
        Resource::Assessment(m) => &m.id,
        Resource::Link(m) => &m.id,
    }
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

impl SchoologyAccountConfig {
    /// Fetch a complete Schoology material tree as a standardized folder.
    pub fn course(&self, section_id: &str, folder_id: &str) -> RequestResult<Folder> {
        info!("scraping Schoology course tree: course={section_id}, folder={folder_id}");
        let mut folder = self.scrape_folder(
            section_id,
            folder_id,
            None,
            &mut HashSet::new(),
            None,
            &HashSet::new(),
        )?;
        folder.set_folder_id(section_id);
        Ok(folder)
    }

    /// Refresh placement and fetch only notified resources absent from the cache.
    pub fn hierarchy(
        &self,
        section_id: &str,
        existing: &[Resource],
        posted: &HashSet<String>,
    ) -> RequestResult<Folder> {
        let cached = existing
            .iter()
            .map(|m| {
                (
                    (
                        ResourceType::from(m),
                        resource_id(m).to_owned(),
                    ),
                    m.clone(),
                )
            })
            .collect();
        let mut folder = self.scrape_folder(
            section_id,
            "0",
            None,
            &mut HashSet::new(),
            Some(&cached),
            posted,
        )?;
        folder.set_folder_id(section_id);
        Ok(folder)
    }

    fn scrape_folder(
        &self,
        section_id: &str,
        folder_id: &str,
        url: Option<&str>,
        visited: &mut HashSet<String>,
        cached: Option<&HashMap<(ResourceType, String), Resource>>,
        posted: &HashSet<String>,
    ) -> RequestResult<Folder> {
        foam::thread_manager::check_cancelled()?;
        if !visited.insert(folder_id.to_owned()) {
            return Err(io::Error::other(format!("course folder cycle at {folder_id}")).into());
        }
        let fallback_url = format!("{API_ROOT}/{section_id}/folder/{folder_id}");
        let raw: RawFolderResponse = self.api_get(url.unwrap_or(&fallback_url))?;
        let meta = CourseMaterial::from_raw(raw.self_folder)?;
        let mut folder = Folder {
            id: meta.id,
            title: meta.title,
            body: meta.body,
            resources: Vec::with_capacity(raw.folder_items.len()),
        };
        for item in raw.folder_items {
            let material = CourseMaterial::from_raw(item)?;
            if material.material_type == "folder" {
                let child = self.scrape_folder(
                    section_id,
                    &material.id,
                    material.location.as_deref(),
                    visited,
                    cached,
                    posted,
                )?;
                folder.resources.push(Resource::Folder(Box::new(child)));
            } else if let Some(cached) = cached {
                let kind = match material.material_type.as_str() {
                    "assignment" => Some(ResourceType::Task),
                    "document" => Some(ResourceType::Document),
                    "link" => Some(ResourceType::Link),
                    "assessment" | "test/quiz" | "quiz" => Some(ResourceType::Assessment),
                    _ => None,
                };
                if let Some(value) = kind.and_then(|kind| cached.get(&(kind, material.id.clone())))
                {
                    folder.resources.push(value.clone());
                } else if posted.contains(&material.id) {
                    match self.scrape_material(&material) {
                        Ok(Some(value)) => folder.resources.push(value),
                        Ok(None) => {}
                        Err(err) => {
                            log::warn!("Fetching posted material {} failed: {err}", material.id)
                        }
                    }
                }
            } else {
                if let Some(resource) = self.scrape_material(&material)? {
                    folder.resources.push(resource);
                }
            }
        }
        Ok(folder)
    }
}
