use std::{io, path::Path};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::types::material::Material;

use super::write_json;

pub(super) fn write_material(path: &Path, material: &Material) -> io::Result<()> {
    match material {
        Material::Folder(value) => write_typed(path, "folder", value),
        Material::Assessment(value) => write_typed(path, "assessment", value),
        Material::Assignment(value) => write_typed(path, "assignment", value),
        Material::Document(value) => write_typed(path, "document", value),
        Material::Link(value) => write_typed(path, "link", value),
    }
}

fn write_typed(path: &Path, material_type: &str, material: &impl Serialize) -> io::Result<()> {
    let fields = serde_json::to_value(material).map_err(io::Error::other)?;
    let Value::Object(fields) = fields else {
        return Err(io::Error::other("material did not serialize as an object"));
    };
    let mut object = Map::new();
    object.insert("type".to_owned(), Value::String(material_type.to_owned()));
    object.extend(fields);
    write_json(path, &Value::Object(object))
}
