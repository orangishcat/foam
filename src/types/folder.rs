use std::iter::once;

use serde::{Deserialize, Serialize};

use super::material::Material;

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

impl Folder {
    /// Associate assignments, including nested ones, with their containing course.
    pub fn set_course_id(&mut self, course_id: &str) {
        for material in &mut self.materials {
            match material {
                Material::Folder(folder) => folder.set_course_id(course_id),
                Material::Assignment(assignment) => assignment.course_id = course_id.to_owned(),
                _ => {}
            }
        }
    }

    pub fn recursive_iter(&self) -> Box<dyn Iterator<Item = &Material> + '_> {
        Box::new(self.materials.iter().flat_map(|material| match material {
            Material::Folder(folder_box) => folder_box.recursive_iter(),
            _ => Box::new(once(material)),
        }))
    }
}
