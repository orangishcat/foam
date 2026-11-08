use anyhow;
use std::{
    io,
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock},
};

use rusqlite::Connection;
use serde::de::DeserializeOwned;

use crate::config::config;

static DB_CONNECTION: OnceLock<Mutex<Connection>> = OnceLock::new();
const DB_NAME: &str = "foam.db";

pub fn init() -> anyhow::Result<()> {
    let connection = Connection::open(db_path())?;
    connection.create_collation("NATURAL_ORDER", natord::compare_ignore_case)?;
    DB_CONNECTION
        .set(Mutex::new(connection))
        .map_err(|_| io::Error::other("setting oncelock failed"))?;
    create_schema()?;
    Ok(())
}

pub fn db_path() -> PathBuf {
    config().data_dir().join(DB_NAME)
}

fn create_schema() -> anyhow::Result<()> {
    connection()?.execute_batch(include_str!("sql/schema.sql"))?;
    add_column_if_missing("materials", "last_updated", "TEXT")?;
    add_column_if_missing("notifications", "subtitle", "TEXT NOT NULL DEFAULT \"\"")?;
    add_column_if_missing("notifications", "seq", "INTEGER NOT NULL DEFAULT 0")?;
    Ok(())
}

pub fn connection() -> anyhow::Result<MutexGuard<'static, Connection>> {
    let guard = DB_CONNECTION
        .get()
        .ok_or_else(|| io::Error::other("database is not initialized"))?;
    guard
        .lock()
        .map_err(|_| io::Error::other("database lock is poisoned").into())
}

pub fn from_sql<M: DeserializeOwned>(
    query: String,
    params: &[&dyn rusqlite::ToSql],
) -> anyhow::Result<Vec<M>> {
    let connection = connection()?;
    read_from(&connection, &query, params)
}

pub(crate) fn read_from<M: DeserializeOwned>(
    connection: &Connection,
    query: &str,
    params: &[&dyn rusqlite::ToSql],
) -> anyhow::Result<Vec<M>> {
    let mut statement = connection.prepare_cached(query)?;
    let rows = statement.query(params)?;
    Ok(serde_rusqlite::from_rows::<M>(rows).collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn from_sql_map<T>(
    query: String,
    params: &[&dyn rusqlite::ToSql],
    mut map_row: impl FnMut(&rusqlite::Row) -> anyhow::Result<T>,
) -> anyhow::Result<Vec<T>> {
    let connection = connection()?;
    let mut statement = connection.prepare_cached(&query)?;
    let mut rows = statement.query(params)?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(map_row(row)?);
    }
    Ok(results)
}

pub fn bulk_execute<P>(query: String, params: Vec<P>) -> anyhow::Result<usize>
where
    P: rusqlite::Params,
{
    let mut connection = connection()?;
    bulk_execute_on(&mut connection, &query, params)
}

pub(crate) fn bulk_execute_on<P: rusqlite::Params>(
    connection: &mut Connection,
    query: &str,
    params: Vec<P>,
) -> anyhow::Result<usize> {
    let mut updated = 0;
    let transaction = connection.transaction()?;
    let mut statement = transaction.prepare_cached(query)?;
    for entry in params {
        updated += statement.execute(entry)?;
    }
    drop(statement);
    transaction.commit()?;
    Ok(updated)
}

pub fn execute(query: String, params: &[&dyn rusqlite::ToSql]) -> anyhow::Result<usize> {
    let connection = connection()?;
    Ok(connection.prepare_cached(&query)?.execute(params)?)
}

// for use in database migration only, function will be unused most of the time
pub fn add_column_if_missing(table: &str, column: &str, definition: &str) -> anyhow::Result<bool> {
    let connection = connection()?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo(?1) WHERE name = ?2)",
        (table, column),
        |row| Ok(row.get(0)?),
    )?;
    if exists {
        return Ok(false);
    }

    connection.execute(
        &format!("ALTER TABLE \"{table}\" ADD COLUMN \"{column}\" {definition}"),
        [],
    )?;
    Ok(true)
}
