use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

use log::{info, warn};
use serde::Deserialize;
use serde_json::Value;

use super::{RequestResult, api_get};
pub mod courses;
pub mod materials;

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
/// Material details are stored below the containing folder using sanitized,
/// deduplicated titles rather than Schoology IDs.
///
/// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/course-folder/>
pub fn course(course_id: &str, folder_id: &str, course_dir: &Path) {
    info!("scraping Schoology course folder tree: course={course_id}, folder={folder_id}");
    let mut visited = HashSet::new();
    let url = format!("{API_ROOT}/{course_id}/folder/{folder_id}");
    if let Err(error) = scrape_folder(course_id, folder_id, &url, course_dir, &mut visited) {
        warn!("skipping failed Schoology course scrape: course={course_id}, error={error}");
    }
}

/// Scrapes one Course Folder response and recursively follows child folders.
///
/// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/course-folder/>
fn scrape_folder(
    course_id: &str,
    folder_id: &str,
    url: &str,
    folder_dir: &Path,
    visited: &mut HashSet<String>,
) -> RequestResult<Option<CourseFolderResponse>> {
    if !visited.insert(folder_id.to_owned()) {
        warn!("skipping already visited course folder: course={course_id}, folder={folder_id}");
        return Ok(None);
    }

    info!("scraping Schoology course folder: course={course_id}, folder={folder_id}, url={url}");
    let raw_response: Value = api_get(url)?;
    let response: RawFolderResponse = serde_json::from_value(raw_response)?;
    let materials_dir = folder_dir.join("materials");
    fs::create_dir_all(&materials_dir)?;

    let folder_items: Vec<CourseMaterial> = response
        .folder_items
        .into_iter()
        .map(CourseMaterial::from_raw)
        .collect::<RequestResult<_>>()?;

    for material in &folder_items {
        if material.material_type.as_deref() == Some("folder") {
            if visited.contains(&material.id) {
                warn!(
                    "skipping already visited child folder: course={course_id}, folder={}",
                    material.id
                );
                continue;
            }
            let child_url = material
                .location
                .clone()
                .unwrap_or_else(|| format!("{API_ROOT}/{course_id}/folder/{}", material.id));
            match deduplicated_folder(folder_dir, &material.title).and_then(|child_dir| {
                scrape_folder(course_id, &material.id, &child_url, &child_dir, visited)
            }) {
                Ok(_) => {}
                Err(error) => warn!(
                    "skipping failed Schoology folder: course={course_id}, folder={}, error={error}",
                    material.id
                ),
            }
        } else {
            if let Err(error) = materials::scrape(material, &materials_dir) {
                warn!(
                    "skipping failed Schoology material: course={course_id}, material={}, error={error}",
                    material.id
                );
            }
        }
    }

    Ok(Some(CourseFolderResponse {
        self_folder: CourseMaterial::from_raw(response.self_folder)?,
        parent: response.parent.map(CourseMaterial::from_raw).transpose()?,
        folder_items,
    }))
}

impl CourseMaterial {
    fn from_raw(raw: Value) -> RequestResult<Self> {
        let id = scalar_as_string(
            raw.get("id")
                .ok_or_else(|| io::Error::other("Schoology course material is missing an id"))?,
        )
        .ok_or_else(|| {
            io::Error::other("Schoology course material id is not a string or number")
        })?;

        Ok(Self {
            id,
            title: string_field(&raw, "title"),
            body: string_field(&raw, "body"),
            material_type: raw.get("type").and_then(Value::as_str).map(str::to_owned),
            location: raw
                .get("location")
                .and_then(Value::as_str)
                .map(str::to_owned),
            raw,
        })
    }
}

pub(super) fn deduplicated_folder(parent: &Path, title: &str) -> RequestResult<PathBuf> {
    let stem = materials::safe_file_stem(title);
    let mut duplicate = 1usize;
    loop {
        let name = if duplicate == 1 {
            stem.clone()
        } else {
            format!("{stem} ({duplicate})")
        };
        let path = parent.join(name);
        match fs::create_dir(&path) {
            Ok(()) => {
                info!("created Schoology course folder: {}", path.display());
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => duplicate += 1,
            Err(error) => return Err(error.into()),
        }
    }
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
