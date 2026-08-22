use log::{info, warn};

use super::CourseMaterial;
use crate::{
    schoology::{RequestResult, api_get},
    types::material::Material,
};

pub mod assessment;
pub mod assignment;
pub mod discussion;
pub mod document;
pub mod external_tool;
pub mod link;
pub mod media_album;
pub mod package;
pub mod types;
pub mod web_package;

/// Fetch and coerce a Schoology material into the provider-independent model.
pub fn scrape(material: &CourseMaterial) -> RequestResult<Option<Material>> {
    let material_type = material.material_type.as_str();
    info!(
        "scraping Schoology material: id={}, type={material_type}",
        material.id
    );
    let Some(url) = material.location.as_deref() else {
        warn!(
            "skipping Schoology material without an API location: {}",
            material.id
        );
        return Ok(None);
    };

    Ok(Some(match material_type {
        "assignment" => Material::Assignment(assignment::scrape(material, url)?),
        "document" => Material::Document(document::scrape(material, url)?),
        "assessment" | "test/quiz" | "quiz" => {
            Material::Assessment(assessment::scrape(material, url)?)
        }
        "link" => Material::Link(link::scrape(material, url)?),
        _ => {
            warn!("skipping unsupported Schoology material type: {material_type}");
            return Ok(None);
        }
    }))
}
