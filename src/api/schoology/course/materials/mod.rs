use crate::api::schoology::account::SchoologyAccountConfig;
use log::{info, warn};

use super::CourseMaterial;
use crate::{api::schoology::RequestResult, types::material::Material};

pub mod assessment;
pub mod assignment;
pub mod discussion;
pub mod document;
pub mod external_tool;
pub mod link;
pub mod media_album;
pub mod package;
pub mod page;
pub mod types;
pub mod web_package;

impl SchoologyAccountConfig {
    /// Fetch and coerce a Schoology material into the provider-independent model.
    pub fn scrape_material(&self, material: &CourseMaterial) -> RequestResult<Option<Material>> {
        let material_type = material.material_type.as_str();
        crate::thread_manager::check_cancelled()?;
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
            "assignment" => Material::Assignment(self.scrape_assignment(material, url)?),
            "document" => Material::Document(self.scrape_document(material, url)?),
            "assessment" | "test/quiz" | "quiz" => {
                Material::Assessment(self.scrape_assessment(material, url)?)
            }
            "link" => Material::Link(self.scrape_link(material, url)?),
            _ => {
                warn!("skipping unsupported Schoology material type: {material_type}");
                return Ok(None);
            }
        }))
    }
}
