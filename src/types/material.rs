use serde::{Deserialize, Serialize};

use super::{
    assessment::Assessment, assignment::Assignment, document::Document, folder::Folder, link::Link,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "material", rename_all = "snake_case")]
pub enum Material {
    Folder(Box<Folder>),
    Assessment(Assessment),
    Assignment(Assignment),
    Document(Document),
    Link(Link),
}

/// The kind of a material, without its resource data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialType {
    Folder,
    Assessment,
    Assignment,
    Document,
    Link,
}

impl From<&Material> for MaterialType {
    fn from(material: &Material) -> Self {
        match material {
            Material::Folder(_) => Self::Folder,
            Material::Assessment(_) => Self::Assessment,
            Material::Assignment(_) => Self::Assignment,
            Material::Document(_) => Self::Document,
            Material::Link(_) => Self::Link,
        }
    }
}
