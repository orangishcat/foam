use crate::api::schoology::account::SchoologyAccountConfig;
use log::{info, warn};

use super::CourseMaterial;
use crate::api::schoology::RequestResult;
use foam::types::resource::Resource;

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

        Ok(Some(match material_type {
            "assignment" => Resource::Task(self.scrape_assignment(material, url)?),
            "document" => Resource::Document(self.scrape_document(material, url)?),
            "assessment" | "test/quiz" | "quiz" => {
                Resource::Assessment(self.scrape_assessment(material, url)?)
            }
            "link" => Resource::Link(self.scrape_link(material, url)?),
            _ => {
                warn!("skipping unsupported Schoology material type: {material_type}");
                return Ok(None);
            }
        }))
    }
}
