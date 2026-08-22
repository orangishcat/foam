use std::{fs, io, path::Path};

use serde::Serialize;

use crate::types::{folder::Folder, material::Material};

use super::{material::write_material, material_title, unique_directory, unique_file, write_json};

#[derive(Serialize)]
struct FolderMetadata<'a> {
    #[serde(rename = "type")]
    material_type: &'static str,
    id: &'a str,
    title: &'a str,
    body: &'a str,
}

pub(super) fn write_folder_contents(folder: &Folder, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    write_json(
        &destination.join("folder.json"),
        &FolderMetadata {
            material_type: "folder",
            id: &folder.id,
            title: &folder.title,
            body: &folder.body,
        },
    )?;

    for material in &folder.materials {
        match material {
            Material::Folder(folder) => {
                let child = unique_directory(destination, &folder.title)?;
                write_folder_contents(folder, &child)?;
            }
            material => {
                let path = unique_file(destination, material_title(material), "json");
                write_material(&path, material)?;
            }
        }
    }
    Ok(())
}
