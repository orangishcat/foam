use std::iter::once;

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

    pub fn recursive_iter(&self) -> Box<dyn Iterator<Item = &Material> + '_> {
        Box::new(self.materials.iter().flat_map(|material| match material {
            Material::Folder(folder_box) => folder_box.recursive_iter(),
            _ => Box::new(once(material)),
        }))
    }

    pub fn recursive_iter_mut(&mut self) -> Box<dyn Iterator<Item = &mut Material> + '_> {
        Box::new(
            self.materials
                .iter_mut()
                .flat_map(|material| match material {
                    Material::Folder(folder) => folder.recursive_iter_mut(),
                    _ => Box::new(once(material)),
                }),
        )
    }
}
