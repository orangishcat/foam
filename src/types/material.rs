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
