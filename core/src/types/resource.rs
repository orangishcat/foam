use crate::database;
use anyhow;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::{assessment::Assessment, document::Document, folder::Folder, link::Link, task::Task};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "resource", rename_all = "snake_case")]
pub enum Resource {
    Folder(Box<Folder>),
    Assessment(Assessment),
    Task(Task),
    Document(Document),
    Link(Link),
}

/// The kind of a resource, without its resource data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Folder,
    Assessment,
    Task,
    Document,
    Link,
}

impl From<&Resource> for ResourceType {
    fn from(resource: &Resource) -> Self {
        match resource {
            Resource::Folder(_) => Self::Folder,
            Resource::Assessment(_) => Self::Assessment,
            Resource::Task(_) => Self::Task,
            Resource::Document(_) => Self::Document,
            Resource::Link(_) => Self::Link,
        }
    }
}

pub fn store_hierarchy(folder_id: &str, root: &Folder) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    store_hierarchy_on(&transaction, folder_id, root)?;
    Ok(transaction.commit()?)
}

/// Insert one task without replacing the rest of the folder hierarchy.
pub fn store_task(
    folder_id: &str,
    parent_id: &str,
    task: &Task,
) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    store_resource_on(
        &transaction,
        folder_id,
        parent_id,
        &Resource::Task(task.clone()),
    )?;
    Ok(transaction.commit()?)
}

/// Add calendar tasks and their folder paths without pruning other resources.
pub fn store_calendar_tasks(
    folder_id: &str,
    root: &Folder,
    wanted: &std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    store_calendar_folder_on(&transaction, folder_id, "", root, wanted)?;
    Ok(transaction.commit()?)
}

fn store_calendar_folder_on(
    connection: &Connection,
    folder_id: &str,
    parent_id: &str,
    folder: &Folder,
    wanted: &std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    let metadata = Folder {
        id: folder.id.clone(),
        title: folder.title.clone(),
        body: folder.body.clone(),
        resources: Vec::new(),
    };
    store_resource_on(
        connection,
        folder_id,
        parent_id,
        &Resource::Folder(Box::new(metadata)),
    )?;
    for resource in &folder.resources {
        match resource {
            Resource::Folder(child) => {
                store_calendar_folder_on(connection, folder_id, &folder.id, child, wanted)?;
            }
            Resource::Task(task) if wanted.contains(&task.id) => {
                store_resource_on(connection, folder_id, &folder.id, resource)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn store_hierarchy_on(
    connection: &Connection,
    folder_id: &str,
    root: &Folder,
) -> anyhow::Result<()> {
    let mut retained = Vec::new();
    store_folder_on(connection, folder_id, "", root, &mut retained)?;
    connection.execute(
        "DELETE FROM resources WHERE folder_id = ?1 AND NOT EXISTS (
            SELECT 1 FROM json_each(?2) WHERE json_extract(value, '$[0]') = resources.type
                AND json_extract(value, '$[1]') = resources.resource_id)",
        params![folder_id, serde_json::to_string(&retained)?],
    )?;
    Ok(())
}

fn store_folder_on(
    connection: &Connection,
    folder_id: &str,
    parent_id: &str,
    folder: &Folder,
    retained: &mut Vec<(ResourceType, String)>,
) -> anyhow::Result<()> {
    let metadata = Folder {
        id: folder.id.clone(),
        title: folder.title.clone(),
        body: folder.body.clone(),
        resources: Vec::new(),
    };
    store_resource_on(
        connection,
        folder_id,
        parent_id,
        &Resource::Folder(Box::new(metadata)),
    )?;
    retained.push((ResourceType::Folder, folder.id.clone()));
    for resource in &folder.resources {
        if let Resource::Folder(child) = resource {
            store_folder_on(connection, folder_id, &folder.id, child, retained)?;
        } else {
            store_resource_on(connection, folder_id, &folder.id, resource)?;
            retained.push((ResourceType::from(resource), resource.id().to_owned()));
        }
    }
    Ok(())
}

fn store_resource_on(
    connection: &Connection,
    folder_id: &str,
    parent_id: &str,
    resource: &Resource,
) -> anyhow::Result<()> {
    let (id, title) = match resource {
        Resource::Folder(m) => (&m.id, &m.title),
        Resource::Task(m) => (&m.id, &m.title),
        Resource::Assessment(m) => (&m.id, &m.title),
        Resource::Document(m) => (&m.id, &m.title),
        Resource::Link(m) => (&m.id, &m.title),
    };
    let kind = serde_json::to_value(ResourceType::from(resource))?;
    // Task content lives only in tasks; data holds other resource variants.
    let data = if matches!(resource, Resource::Task(_)) {
        "{}".to_owned()
    } else {
        serde_json::to_string(resource)?
    };
    let source = crate::plugin::host::attribution();
    connection.execute(
        "INSERT INTO resources (folder_id, resource_id, type, parent_id, source, title, data) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(folder_id, type, resource_id) DO UPDATE SET parent_id = excluded.parent_id, source = excluded.source,
         title = CASE WHEN excluded.type = 'task' THEN resources.title ELSE excluded.title END, data = excluded.data",
        params![folder_id, id, kind.as_str(), parent_id, source, title, data])?;
    if let Resource::Task(task) = resource {
        task.insert_on(connection, folder_id)?;
    }
    Ok(())
}

pub fn resources(folder_id: &str) -> anyhow::Result<Vec<Resource>> {
    let connection = database::connection()?;
    resources_from(&connection, folder_id)
}

fn resources_from(connection: &Connection, folder_id: &str) -> anyhow::Result<Vec<Resource>> {
    let mut statement = connection.prepare_cached(
        "SELECT data FROM resources WHERE folder_id = ? AND type != 'task'",
    )?;
    let rows = statement.query_map(params![folder_id], |row| Ok(row.get::<_, String>(0)?))?;
    let mut resources = rows
        .map(|row| Ok(serde_json::from_str(&row?)?))
        .collect::<anyhow::Result<Vec<_>>>()?;
    resources.extend(
        database::read_from::<Task>(
            connection,
            "SELECT * FROM tasks WHERE folder_id = ?",
            params![folder_id],
        )?
        .into_iter()
        .map(Resource::Task),
    );
    Ok(resources)
}

impl Resource {
    pub fn id(&self) -> &str {
        match self {
            Self::Folder(m) => &m.id,
            Self::Task(m) => &m.id,
            Self::Assessment(m) => &m.id,
            Self::Document(m) => &m.id,
            Self::Link(m) => &m.id,
        }
    }
}
