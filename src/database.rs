use std::{
    io::{self, Error, Result},
    sync::{Mutex, MutexGuard, OnceLock},
};

use rusqlite::Connection;
use serde::de::DeserializeOwned;

use crate::config::config;

static DB_CONNECTION: OnceLock<Mutex<Connection>> = OnceLock::new();
const DB_NAME: &str = "foam.db";

pub fn init() -> Result<()> {
    let data_dir = config().data_dir().to_owned();
    let connection = Connection::open(data_dir.join(DB_NAME)).map_err(Error::other)?;
    DB_CONNECTION
        .set(Mutex::new(connection))
        .map_err(|_| Error::other("setting oncelock failed"))?;
    create_schema()?;
    Ok(())
}

fn create_schema() -> Result<()> {
    connection()?
        .execute_batch(include_str!("sql/schema.sql"))
        .map_err(Error::other)?;
    Ok(())
}

pub fn connection() -> Result<MutexGuard<'static, Connection>> {
    DB_CONNECTION
        .get()
        .ok_or_else(|| Error::other("database is not initialized"))?
        .lock()
        .map_err(|_| Error::other("database lock is poisoned"))
}

pub fn from_sql<M: DeserializeOwned>(
    query: String,
    params: &[&dyn rusqlite::ToSql],
) -> io::Result<Vec<M>> {
    let connection = connection()?;
    read_from(&connection, &query, params)
}

pub(crate) fn read_from<M: DeserializeOwned>(
    connection: &Connection,
    query: &str,
    params: &[&dyn rusqlite::ToSql],
) -> io::Result<Vec<M>> {
    let mut statement = connection.prepare_cached(query).map_err(Error::other)?;
    let rows = statement.query(params).map_err(Error::other)?;
    serde_rusqlite::from_rows::<M>(rows)
        .map(|r| r.map_err(Error::other))
        .collect::<Result<Vec<M>>>()
}

pub fn from_sql_map<T>(
    query: String,
    params: &[&dyn rusqlite::ToSql],
    mut map_row: impl FnMut(&rusqlite::Row) -> io::Result<T>,
) -> io::Result<Vec<T>> {
    let connection = connection()?;
    let mut statement = connection.prepare_cached(&query).map_err(Error::other)?;
    let mut rows = statement.query(params).map_err(Error::other)?;
    let mut results = Vec::new();
    while let Some(row) = rows.next().map_err(Error::other)? {
        results.push(map_row(row)?);
    }
    Ok(results)
}

pub fn bulk_execute<P>(query: String, params: Vec<P>) -> io::Result<usize>
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
) -> io::Result<usize> {
    let mut updated = 0;
    let transaction = connection.transaction().map_err(Error::other)?;
    let mut statement = transaction.prepare_cached(query).map_err(Error::other)?;
    for entry in params {
        updated += statement.execute(entry).map_err(Error::other)?;
    }
    drop(statement);
    transaction.commit().map_err(Error::other)?;
    Ok(updated)
}

pub fn execute(query: String, params: &[&dyn rusqlite::ToSql]) -> io::Result<usize> {
    let connection = connection()?;
    connection
        .prepare_cached(&query)
        .map_err(Error::other)?
        .execute(params)
        .map_err(Error::other)
}

// for use in database migration only, function will be unused most of the time
pub fn add_column_if_missing(table: &str, column: &str, definition: &str) -> io::Result<bool> {
    let connection = connection()?;
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_xinfo(?1) WHERE name = ?2)",
            (table, column),
            |row| row.get(0),
        )
        .map_err(Error::other)?;
    if exists {
        return Ok(false);
    }

    connection
        .execute(
            &format!("ALTER TABLE \"{table}\" ADD COLUMN \"{column}\" {definition}"),
            [],
        )
        .map_err(Error::other)?;
    Ok(true)
}
