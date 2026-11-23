use crate::database;
use anyhow;
use rusqlite::{Connection, OptionalExtension, params};
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

pub fn store_hierarchy(course_id: &str, root: &Folder) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    store_hierarchy_on(&transaction, course_id, root)?;
    Ok(transaction.commit()?)
}

/// Replace one folder's immediate materials without refreshing subfolder contents.
pub fn store_refreshed_folder(course_id: &str, folder: &Folder) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    let parent_id = transaction
        .query_row(
            "SELECT parent_id FROM materials
             WHERE course_id = ? AND type = 'folder' AND material_id = ?",
            params![course_id, folder.id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_default();
    let last_updated = chrono::Utc::now().to_rfc3339();
    store_material_on(
        &transaction,
        course_id,
        &parent_id,
        &Material::Folder(Box::new(Folder {
            id: folder.id.clone(),
            title: folder.title.clone(),
            body: folder.body.clone(),
            materials: Vec::new(),
        })),
    )?;
    transaction.execute(
        "UPDATE materials SET last_updated = ?
         WHERE course_id = ? AND type = 'folder' AND material_id = ?",
        params![last_updated, course_id, folder.id],
    )?;
    let mut retained = Vec::with_capacity(folder.materials.len());
    for material in &folder.materials {
        if let Material::Folder(child) = material {
            store_child_folder_on(&transaction, course_id, &folder.id, child)?;
        } else {
            store_material_on(&transaction, course_id, &folder.id, material)?;
        }
        retained.push((MaterialType::from(material), material.id().to_owned()));
    }
    prune_folder_children(&transaction, course_id, &folder.id, &retained)?;
    Ok(transaction.commit()?)
}

/// Add a subfolder discovered in a shallow refresh without overwriting its cached contents.
fn store_child_folder_on(
    connection: &Connection,
    course_id: &str,
    parent_id: &str,
    folder: &Folder,
) -> anyhow::Result<()> {
    let metadata = Material::Folder(Box::new(Folder {
        id: folder.id.clone(),
        title: folder.title.clone(),
        body: folder.body.clone(),
        materials: Vec::new(),
    }));
    connection.execute(
        "INSERT INTO materials (course_id, material_id, type, parent_id, title, data)
         VALUES (?1, ?2, 'folder', ?3, ?4, ?5)
         ON CONFLICT(course_id, type, material_id) DO UPDATE SET parent_id = excluded.parent_id",
        params![
            course_id,
            folder.id,
            parent_id,
            folder.title,
            serde_json::to_string(&metadata)?
        ],
    )?;
    Ok(())
}

/// Insert one assignment without replacing the rest of the course hierarchy.
pub fn store_assignment(
    course_id: &str,
    parent_id: &str,
    assignment: &Assignment,
) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    store_material_on(
        &transaction,
        course_id,
        parent_id,
        &Material::Assignment(assignment.clone()),
    )?;
    Ok(transaction.commit()?)
}

/// Add calendar assignments and their folder paths without pruning other materials.
pub fn store_calendar_assignments(
    course_id: &str,
    root: &Folder,
    wanted: &std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    store_calendar_folder_on(&transaction, course_id, "", root, wanted)?;
    Ok(transaction.commit()?)
}

fn store_calendar_folder_on(
    connection: &Connection,
    course_id: &str,
    parent_id: &str,
    folder: &Folder,
    wanted: &std::collections::HashSet<String>,
) -> anyhow::Result<()> {
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
    for material in &folder.materials {
        match material {
            Material::Folder(child) => {
                store_calendar_folder_on(connection, course_id, &folder.id, child, wanted)?;
            }
            Material::Assignment(assignment) if wanted.contains(&assignment.id) => {
                store_material_on(connection, course_id, &folder.id, material)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn store_hierarchy_on(
    connection: &Connection,
    course_id: &str,
    root: &Folder,
) -> anyhow::Result<()> {
    store_hierarchy_on_with(connection, course_id, root, None)
}

/// Store a complete scraped course tree and mark every scraped folder as current.
pub(crate) fn store_scraped_hierarchy_on(
    connection: &Connection,
    course_id: &str,
    root: &Folder,
) -> anyhow::Result<()> {
    let last_updated = chrono::Utc::now().to_rfc3339();
    store_hierarchy_on_with(connection, course_id, root, Some(&last_updated))
}

fn store_hierarchy_on_with(
    connection: &Connection,
    course_id: &str,
    root: &Folder,
    last_updated: Option<&str>,
) -> anyhow::Result<()> {
    let mut retained = Vec::new();
    store_folder_on(connection, course_id, "", root, &mut retained, last_updated)?;
    connection.execute(
        "DELETE FROM materials WHERE course_id = ?1 AND NOT EXISTS (
            SELECT 1 FROM json_each(?2) WHERE json_extract(value, '$[0]') = materials.type
                AND json_extract(value, '$[1]') = materials.material_id)",
        params![course_id, serde_json::to_string(&retained)?],
    )?;
    Ok(())
}

fn store_folder_on(
    connection: &Connection,
    course_id: &str,
    parent_id: &str,
    folder: &Folder,
    retained: &mut Vec<(MaterialType, String)>,
    last_updated: Option<&str>,
) -> anyhow::Result<()> {
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
    if let Some(last_updated) = last_updated {
        connection.execute(
            "UPDATE materials SET last_updated = ?
             WHERE course_id = ? AND type = 'folder' AND material_id = ?",
            params![last_updated, course_id, folder.id],
        )?;
    }
    retained.push((MaterialType::Folder, folder.id.clone()));
    for material in &folder.materials {
        if let Material::Folder(child) = material {
            store_folder_on(
                connection,
                course_id,
                &folder.id,
                child,
                retained,
                last_updated,
            )?;
        } else {
            store_material_on(connection, course_id, &folder.id, material)?;
            retained.push((MaterialType::from(material), material.id().to_owned()));
        }
    }
    Ok(())
}

/// Remove immediate children no longer in a refreshed folder, including removed subfolder
/// trees, while leaving retained subfolder contents untouched.
fn prune_folder_children(
    connection: &Connection,
    course_id: &str,
    folder_id: &str,
    retained: &[(MaterialType, String)],
) -> anyhow::Result<()> {
    connection.execute(
        "WITH RECURSIVE
        protected(type, material_id) AS (
            SELECT type, material_id FROM materials
            WHERE course_id = ?1 AND parent_id = ?2 AND type = 'folder'
                AND EXISTS (
                    SELECT 1 FROM json_each(?3)
                    WHERE json_extract(value, '$[0]') = materials.type
                        AND json_extract(value, '$[1]') = materials.material_id
                )
            UNION
            SELECT m.type, m.material_id FROM materials m
            JOIN protected p ON m.course_id = ?1 AND m.parent_id = p.material_id
        ),
        candidates(rowid, type, material_id) AS (
            SELECT rowid, type, material_id FROM materials
            WHERE course_id = ?1 AND parent_id = ?2
            UNION
            SELECT m.rowid, m.type, m.material_id FROM materials m
            JOIN candidates c ON m.course_id = ?1 AND m.parent_id = c.material_id
        )
        DELETE FROM materials WHERE rowid IN (
            SELECT rowid FROM candidates c
            WHERE NOT EXISTS (
                SELECT 1 FROM json_each(?3)
                WHERE json_extract(value, '$[0]') = c.type
                    AND json_extract(value, '$[1]') = c.material_id
            )
            AND NOT EXISTS (
                SELECT 1 FROM protected p
                WHERE p.type = c.type AND p.material_id = c.material_id
            )
        )",
        params![course_id, folder_id, serde_json::to_string(retained)?],
    )?;
    Ok(())
}

fn store_material_on(
    connection: &Connection,
    course_id: &str,
    parent_id: &str,
    material: &Material,
) -> anyhow::Result<()> {
    let (id, title) = match material {
        Material::Folder(m) => (&m.id, &m.title),
        Material::Assignment(m) => (&m.id, &m.title),
        Material::Assessment(m) => (&m.id, &m.title),
        Material::Document(m) => (&m.id, &m.title),
        Material::Link(m) => (&m.id, &m.title),
    };
    let kind = serde_json::to_value(MaterialType::from(material))?;
    // Assignment content lives only in assignments; data holds other material variants.
    let data = if matches!(material, Material::Assignment(_)) {
        "{}".to_owned()
    } else {
        serde_json::to_string(material)?
    };
    connection.execute(
        "INSERT INTO materials (course_id, material_id, type, parent_id, title, data) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(course_id, type, material_id) DO UPDATE SET parent_id = excluded.parent_id,
         title = CASE WHEN excluded.type = 'assignment' THEN materials.title ELSE excluded.title END, data = excluded.data",
        params![course_id, id, kind.as_str(), parent_id, title, data])?;
    if let Material::Assignment(assignment) = material {
        assignment.insert_on(connection, course_id)?;
    }
    Ok(())
}

pub fn materials(course_id: &str) -> anyhow::Result<Vec<Material>> {
    let connection = database::connection()?;
    materials_from(&connection, course_id)
}

fn materials_from(connection: &Connection, course_id: &str) -> anyhow::Result<Vec<Material>> {
    let mut statement = connection.prepare_cached(
        "SELECT data FROM materials WHERE course_id = ? AND type != 'assignment'",
    )?;
    let rows = statement.query_map(params![course_id], |row| row.get::<_, String>(0))?;
    let mut materials = rows
        .map(|row| Ok(serde_json::from_str(&row?)?))
        .collect::<anyhow::Result<Vec<_>>>()?;
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
