use serde::{Deserialize, Serialize};

use super::material::Material;

/// A provider-independent course folder. Nested folders are boxed by
/// `Material::Folder`, making recursive course trees finite-sized and directly
/// traversable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Folder {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub materials: Vec<Material>,
}
