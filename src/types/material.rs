use crate::database;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::io::{Error, Result};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

pub fn store_hierarchy(course_id: &str, root: &Folder) -> Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction().map_err(Error::other)?;
    store_hierarchy_on(&transaction, course_id, root)?;
    transaction.commit().map_err(Error::other)
}

pub(crate) fn store_hierarchy_on(
    connection: &Connection,
    course_id: &str,
    root: &Folder,
) -> Result<()> {
    let mut retained = Vec::new();
    store_folder_on(connection, course_id, "", root, &mut retained)?;
    connection
        .execute(
            "DELETE FROM materials WHERE course_id = ?1 AND NOT EXISTS (
            SELECT 1 FROM json_each(?2) WHERE json_extract(value, '$[0]') = materials.type
                AND json_extract(value, '$[1]') = materials.material_id)",
            params![
                course_id,
                serde_json::to_string(&retained).map_err(Error::other)?
            ],
        )
        .map_err(Error::other)?;
    Ok(())
}

fn store_folder_on(
    connection: &Connection,
    course_id: &str,
    parent_id: &str,
    folder: &Folder,
    retained: &mut Vec<(MaterialType, String)>,
) -> Result<()> {
    let metadata = Folder {
        id: folder.id.clone(),
        title: folder.title.clone(),
        body: folder.body.clone(),
        materials: Vec::new(),
    };
    store_material_on(
        connection,
        course_id,
        parent_id,
        &Material::Folder(Box::new(metadata)),
    )?;
    retained.push((MaterialType::Folder, folder.id.clone()));
    for material in &folder.materials {
        if let Material::Folder(child) = material {
            store_folder_on(connection, course_id, &folder.id, child, retained)?;
        } else {
            store_material_on(connection, course_id, &folder.id, material)?;
            retained.push((MaterialType::from(material), material.id().to_owned()));
        }
    }
    Ok(())
}

fn store_material_on(
    connection: &Connection,
    course_id: &str,
    parent_id: &str,
    material: &Material,
) -> Result<()> {
    let (id, title) = match material {
        Material::Folder(m) => (&m.id, &m.title),
        Material::Assignment(m) => (&m.id, &m.title),
        Material::Assessment(m) => (&m.id, &m.title),
        Material::Document(m) => (&m.id, &m.title),
        Material::Link(m) => (&m.id, &m.title),
    };
    let kind = serde_json::to_value(MaterialType::from(material)).map_err(Error::other)?;
    // Assignment content lives only in assignments; data holds other material variants.
    let data = if matches!(material, Material::Assignment(_)) {
        "{}".to_owned()
    } else {
        serde_json::to_string(material).map_err(Error::other)?
    };
    connection.execute(
        "INSERT INTO materials (course_id, material_id, type, parent_id, title, data) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(course_id, type, material_id) DO UPDATE SET parent_id = excluded.parent_id,
         title = CASE WHEN excluded.type = 'assignment' THEN materials.title ELSE excluded.title END, data = excluded.data",
        params![course_id, id, kind.as_str(), parent_id, title, data]).map_err(Error::other)?;
    if let Material::Assignment(assignment) = material {
        assignment.insert_on(connection, course_id)?;
    }
    Ok(())
}

pub fn materials(course_id: &str) -> Result<Vec<Material>> {
    let connection = database::connection()?;
    materials_from(&connection, course_id)
}

fn materials_from(connection: &Connection, course_id: &str) -> Result<Vec<Material>> {
    let mut statement = connection
        .prepare_cached("SELECT data FROM materials WHERE course_id = ? AND type != 'assignment'")
        .map_err(Error::other)?;
    let rows = statement
        .query_map(params![course_id], |row| row.get::<_, String>(0))
        .map_err(Error::other)?;
    let mut materials = rows
        .map(|row| serde_json::from_str(&row.map_err(Error::other)?).map_err(Error::other))
        .collect::<Result<Vec<_>>>()?;
    materials.extend(
        database::read_from::<Assignment>(
            connection,
            "SELECT * FROM assignments WHERE course_id = ?",
            params![course_id],
        )?
        .into_iter()
        .map(Material::Assignment),
    );
    Ok(materials)
}

impl Material {
    pub fn id(&self) -> &str {
        match self {
            Self::Folder(m) => &m.id,
            Self::Assignment(m) => &m.id,
            Self::Assessment(m) => &m.id,
            Self::Document(m) => &m.id,
            Self::Link(m) => &m.id,
        }
    }
}
