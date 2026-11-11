use crate::database;
use anyhow;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_with::{PickFirst, Same, json::JsonString, serde_as};

use super::folder::Folder;

/// Sidebar-level folder metadata. `title`, `folder_id`, and `custom_icon` live
/// in the `folders` table columns; only the generic keys below are serialized
/// into `folders.data`:
/// - `aliases`: cross-source notification -> folder id routing,
/// - `folder_order`: core drag order,
/// - `hidden`: sidebar visibility,
/// - `period`: core UI value,
/// - `description`: generic folder description (indexed by the FTS triggers).
#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderData {
    /// Table columns, not serialized into `data`.
    #[serde(skip)]
    pub folder_id: String,
    #[serde(skip)]
    pub title: String,
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub aliases: Vec<String>,
    /// Populated by a source plugin from its own attachment cache; stored in
    /// the `folders.custom_icon` column, not in `data`. Deserialization
    /// ignores any `custom_icon` key left in legacy `data` JSON.
    #[serde(skip_serializing)]
    pub custom_icon: String,
    pub description: String,
    pub period: Option<String>,
    pub folder_order: u8,
    pub hidden: bool,
}

pub fn folder(id: &str) -> anyhow::Result<Option<FolderData>> {
    let connection = database::connection()?;
    folder_from(&connection, id)
}

/// All folders in sidebar order, with table columns merged into each entry.
pub fn folders() -> anyhow::Result<Vec<FolderData>> {
    let connection = database::connection()?;
    read_folders(
        &connection,
        "SELECT folder_id, title, custom_icon, data FROM folders ORDER BY json_extract(data, '$.folder_order'), title COLLATE NATURAL_ORDER",
        [],
    )
}

pub(crate) fn folder_from(connection: &Connection, id: &str) -> anyhow::Result<Option<FolderData>> {
    let exact = read_folders(
        connection,
        "SELECT folder_id, title, custom_icon, data FROM folders WHERE folder_id = ?",
        params![id],
    )?;
    if let Some(folder) = exact.into_iter().next() {
        return Ok(Some(folder));
    }
    let mut matches = read_folders(
        connection,
        "SELECT folder_id, title, custom_icon, data FROM folders WHERE EXISTS (
            SELECT 1 FROM json_each(folders.data, '$.aliases') WHERE value = ?)",
        params![id],
    )?;
    if matches.len() > 1 {
        return Err(anyhow::anyhow!("ambiguous folder alias"));
    }
    Ok(matches.pop())
}

/// Merge the table columns with the serialized `data` JSON into one FolderData.
fn folder_row(row: &rusqlite::Row) -> anyhow::Result<FolderData> {
    let folder_id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let custom_icon: Option<String> = row.get(2)?;
    let data: String = row.get(3)?;
    let mut info: FolderData = serde_json::from_str(&data)?;
    info.folder_id = folder_id;
    info.title = title;
    info.custom_icon = custom_icon.unwrap_or_default();
    Ok(info)
}

fn read_folders(
    connection: &Connection,
    query: &str,
    params: impl rusqlite::Params,
) -> anyhow::Result<Vec<FolderData>> {
    let mut statement = connection.prepare_cached(query)?;
    let mut rows = statement.query(params)?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(folder_row(row)?);
    }
    Ok(results)
}

pub fn resolve_folder_alias(id: &str, title: &str) -> anyhow::Result<FolderData> {
    let mut connection = database::connection()?;
    resolve_alias_on(&mut connection, id, title)
}

pub(crate) fn resolve_alias_on(
    connection: &mut Connection,
    id: &str,
    title: &str,
) -> anyhow::Result<FolderData> {
    let transaction = connection.transaction()?;
    if let Some(folder) = folder_from(&transaction, id)? {
        return Ok(folder);
    }
    let mut matches = read_folders(
        &transaction,
        "SELECT folder_id, title, custom_icon, data FROM folders WHERE title = ? AND ? != ''",
        params![title, title],
    )?;
    if matches.len() != 1 {
        return Err(anyhow::anyhow!(
            "notification folder title is missing or ambiguous"
        ));
    }
    let mut folder = matches.pop().unwrap();
    folder.aliases.push(id.to_owned());
    write_folder_on(&transaction, &folder)?;
    transaction.commit()?;
    Ok(folder)
}

pub(crate) fn write_folder_on(connection: &Connection, folder: &FolderData) -> anyhow::Result<()> {
    let data = serde_json::to_string(folder)?;
    let source = crate::plugin::host::attribution();
    connection.execute(
        include_str!("../sql/folder/write.sql"),
        params![folder.folder_id, source, folder.title, folder.custom_icon, data],
    )?;
    Ok(())
}

/// Persist a fetched folder and its hierarchy atomically. Existing task state wins.
pub fn store_folder(folder: &FolderData, root: &Folder) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    let mut folder = folder.clone();
    if let Some(current) = read_folders(
        &transaction,
        "SELECT folder_id, title, custom_icon, data FROM folders WHERE folder_id = ?",
        params![folder.folder_id],
    )?
    .into_iter()
    .next()
    {
        // User-owned state (aliases, order, hidden flag, period) survives resyncs.
        folder.aliases = current.aliases;
        folder.period = current.period;
        folder.folder_order = current.folder_order;
        folder.hidden = current.hidden;
        if folder.custom_icon.is_empty() {
            folder.custom_icon = current.custom_icon.clone();
        }
    }
    write_folder_on(&transaction, &folder)?;
    super::resource::store_hierarchy_on(&transaction, &folder.folder_id, root)?;
    Ok(transaction.commit()?)
}
