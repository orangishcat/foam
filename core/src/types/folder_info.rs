use crate::database;
use anyhow;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_with::{PickFirst, Same, json::JsonString, serde_as};

use super::folder::Folder;

/// Sidebar-level folder metadata (formerly `Course`); serialized into `folders.data`.
#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderInfo {
    pub folder_id: String, // in Schoology-era data this is actually the section id
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub aliases: Vec<String>,
    pub title: String,
    pub course_code: String,
    pub course_url: String,
    pub section_title: String,
    pub section_code: String,

    pub active: bool,
    pub description: String,
    pub logo_img_src: String,
    pub location: String,

    pub period: Option<String>,
    pub folder_order: u8,
    pub hidden: bool,

    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub meeting_days: Vec<i8>,
    pub start_time: String,
    pub end_time: String,
    pub weight: String,
}

pub fn folder(id: &str) -> anyhow::Result<Option<FolderInfo>> {
    let connection = database::connection()?;
    folder_from(&connection, id)
}

pub(crate) fn folder_from(connection: &Connection, id: &str) -> anyhow::Result<Option<FolderInfo>> {
    let exact = read_folders(
        connection,
        "SELECT folder_id, title, data FROM folders WHERE folder_id = ?",
        params![id],
    )?;
    if let Some(folder) = exact.into_iter().next() {
        return Ok(Some(folder));
    }
    let mut matches = read_folders(
        connection,
        "SELECT folder_id, title, data FROM folders WHERE EXISTS (
            SELECT 1 FROM json_each(folders.data, '$.aliases') WHERE value = ?)",
        params![id],
    )?;
    if matches.len() > 1 {
        return Err(anyhow::anyhow!("ambiguous folder alias"));
    }
    Ok(matches.pop())
}

/// Merge the table columns with the serialized `data` JSON into one FolderInfo.
fn folder_row(row: &rusqlite::Row) -> anyhow::Result<FolderInfo> {
    let folder_id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let data: String = row.get(2)?;
    let mut info: FolderInfo = serde_json::from_str(&data)?;
    info.folder_id = folder_id;
    info.title = title;
    Ok(info)
}

fn read_folders(
    connection: &Connection,
    query: &str,
    params: impl rusqlite::Params,
) -> anyhow::Result<Vec<FolderInfo>> {
    let mut statement = connection.prepare_cached(query)?;
    let mut rows = statement.query(params)?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(folder_row(row)?);
    }
    Ok(results)
}

pub fn resolve_folder_alias(id: &str, title: &str) -> anyhow::Result<FolderInfo> {
    let mut connection = database::connection()?;
    resolve_alias_on(&mut connection, id, title)
}

pub(crate) fn resolve_alias_on(
    connection: &mut Connection,
    id: &str,
    title: &str,
) -> anyhow::Result<FolderInfo> {
    let transaction = connection.transaction()?;
    if let Some(folder) = folder_from(&transaction, id)? {
        return Ok(folder);
    }
    let mut matches = read_folders(
        &transaction,
        "SELECT folder_id, title, data FROM folders WHERE title = ? AND ? != ''",
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

pub(crate) fn write_folder_on(connection: &Connection, folder: &FolderInfo) -> anyhow::Result<()> {
    let data = serde_json::to_string(folder)?;
    let source = crate::plugin::host::attribution();
    connection.execute(
        include_str!("../sql/folder/write.sql"),
        params![folder.folder_id, source, "", folder.title, data],
    )?;
    Ok(())
}

/// Persist a fetched folder and its hierarchy atomically. Existing task state wins.
pub fn store_folder(folder: &FolderInfo, root: &Folder) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction()?;
    let mut folder = folder.clone();
    if let Some(current) = read_folders(
        &transaction,
        "SELECT folder_id, title, data FROM folders WHERE folder_id = ?",
        params![folder.folder_id],
    )?
    .into_iter()
    .next()
    {
        folder.aliases = current.aliases;
        folder.period = current.period;
    }
    write_folder_on(&transaction, &folder)?;
    super::resource::store_hierarchy_on(&transaction, &folder.folder_id, root)?;
    Ok(transaction.commit()?)
}
