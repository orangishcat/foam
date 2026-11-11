use crate::api::schoology::account::SchoologyAccountConfig;
use log::{info, warn};

use super::CourseMaterial;
use crate::api::schoology::RequestResult;
use foam::types::resource::{Resource, ResourceType};

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
    pub fn scrape_material(&self, material: &CourseMaterial) -> RequestResult<Option<Resource>> {
        let material_type = material.material_type.as_str();
        foam::thread_manager::check_cancelled()?;
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

        let kind_for = |material_type: &str| match material_type {
            "assignment" => Some(ResourceType::Task),
            "document" => Some(ResourceType::Document),
            "link" => Some(ResourceType::Link),
            "assessment" | "test/quiz" | "quiz" => Some(ResourceType::Assessment),
            _ => None,
        };
        Ok(Some(match material_type {
            "assignment" => Resource::Task(self.scrape_assignment(material, url)?),
            "document" => Resource::Document(self.scrape_document(material, url)?),
            "link" => Resource::Link(self.scrape_link(material, url)?),
            _ => {
                if kind_for(material_type).is_none() {
                    warn!("skipping unsupported Schoology material type: {material_type}");
                    return Ok(None);
                }
                // Schoology-only kinds (assessment/test-quiz): payload lives
                // in the plugin's own storage; the core row is a generic
                // labelled shell the views render as title only.
                let response = self.scrape_assessment(material, url)?;
                Resource::Unknown {
                    id: response.id.0,
                    title: response.title,
                    kind: ResourceType::Assessment,
                }
            }
        }))
    }
}
