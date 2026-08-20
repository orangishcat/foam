use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use log::{info, warn};
use serde::Serialize;
use serde_json::Value;

use super::CourseMaterial;
use crate::request::schoology::{RequestResult, api_get};

pub mod assessment;
pub mod assignment;
pub mod discussion;
pub mod document;
pub mod external_tool;
pub mod link;
pub mod media_album;
pub mod package;
pub mod page;
mod types;
pub mod web_package;

/// Scrape any non-folder material exposed by the Course Folder API.
///
/// Schoology supplies the authoritative API endpoint in `location`. The type
/// selects the resource handler; unknown/new types still use that endpoint so
/// additions to the API do not make the scraper silently lose content.
///
/// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/course-folder/>
pub fn scrape(material: &CourseMaterial, destination: &Path) -> RequestResult<PathBuf> {
    let material_type = material.material_type.as_deref().unwrap_or("unknown");
    info!(
        "scraping Schoology course material: id={}, title={}, type={}, location={}",
        material.id,
        material.title,
        material_type,
        material.location.as_deref().unwrap_or("")
    );

    match (material_type, material.location.as_deref()) {
        ("assignment", Some(url)) => assignment::scrape(material, url, destination),
        ("discussion", Some(url)) => discussion::scrape(material, url, destination),
        ("media-album" | "media_album", Some(url)) => {
            media_album::scrape(material, url, destination)
        }
        ("page", Some(url)) => page::scrape(material, url, destination),
        ("document", Some(url)) => document::scrape(material, url, destination),
        ("assessment" | "test/quiz" | "quiz", Some(url)) => {
            assessment::scrape(material, url, destination)
        }
        ("package" | "scorm" | "scorm-package", Some(url)) => {
            package::scrape(material, url, destination)
        }
        ("web-package" | "web_package", Some(url)) => {
            web_package::scrape(material, url, destination)
        }
        ("external-tool" | "external_tool", Some(url)) => {
            external_tool::scrape(material, url, destination)
        }
        ("link", Some(url)) => link::scrape(material, url, destination),
        (_, Some(url)) => {
            warn!(
                "using generic handler for Schoology material type: type={material_type}, url={url}"
            );
            api_material(material, url, destination)
        }
        (_, None) => {
            warn!(
                "material has no API location; saving folder response data: type={material_type}"
            );
            save(material, &material.raw, destination)
        }
    }
}

/// Scrapes a material through the API endpoint supplied in `location`.
///
/// Schoology API overview: <https://developers.schoology.com/api-documentation/rest-api-v1/>
fn api_material(
    material: &CourseMaterial,
    url: &str,
    destination: &Path,
) -> RequestResult<PathBuf> {
    let response: Value = api_get(url)?;
    save(material, &response, destination)
}

fn save<T: Serialize>(
    material: &CourseMaterial,
    value: &T,
    destination: &Path,
) -> RequestResult<PathBuf> {
    fs::create_dir_all(destination)?;
    let contents = serde_json::to_string_pretty(value)?;
    let path = deduplicated_path(destination, &material.title, "json")?;
    fs::write(&path, format!("{contents}\n"))?;
    info!("saved Schoology course material: {}", path.display());
    Ok(path)
}

fn deduplicated_path(directory: &Path, title: &str, extension: &str) -> RequestResult<PathBuf> {
    let stem = safe_file_stem(title);
    let mut duplicate = 1usize;
    loop {
        let name = if duplicate == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({duplicate}).{extension}")
        };
        let path = directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => duplicate += 1,
            Err(error) => return Err(error.into()),
        }
    }
}

pub(super) fn safe_file_stem(title: &str) -> String {
    let stem = title
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'..='\u{1f}' => '_',
            character => character,
        })
        .collect::<String>()
        .trim_matches([' ', '.'])
        .to_owned();

    if stem.is_empty() {
        "untitled".to_owned()
    } else {
        stem
    }
}
