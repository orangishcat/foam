use serde::{Deserialize, Serialize};

use super::resource::Resource;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Folder {
    pub id: String,
    pub title: String,
    pub body: String,
    pub resources: Vec<Resource>,
}

impl Folder {
    pub fn set_folder_id(&mut self, folder_id: &str) {
        for resource in &mut self.resources {
            match resource {
                Resource::Folder(folder) => folder.set_folder_id(folder_id),
                Resource::Task(task) => task.folder_id = folder_id.to_owned(),
                _ => {}
            }
        }
    }
    pub fn folder_id_of(&self) -> &str {
        &self.id
    }
}
