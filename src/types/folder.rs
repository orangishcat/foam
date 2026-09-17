use serde::{Deserialize, Serialize};

use super::material::Material;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Folder {
    pub id: String,
    pub title: String,
    pub body: String,
    pub materials: Vec<Material>,
}

impl Folder {
    pub fn set_course_id(&mut self, course_id: &str) {
        for material in &mut self.materials {
            match material {
                Material::Folder(folder) => folder.set_course_id(course_id),
                Material::Assignment(assignment) => assignment.course_id = course_id.to_owned(),
                _ => {}
            }
        }
    }
}
