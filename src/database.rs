use std::{
    io::{self, Error, Result},
    sync::{Mutex, MutexGuard, OnceLock},
};

use rusqlite::{Connection, fallible_iterator::FallibleIterator};
use serde::de::DeserializeOwned;

use crate::{config::config, types::course::Course};

static DB_CONNECTION: OnceLock<Mutex<Connection>> = OnceLock::new();
const DB_NAME: &str = "foam.db";

pub fn init() -> Result<()> {
    let connection = Connection::open(config().data_dir().join(DB_NAME)).map_err(Error::other)?;
    create_schema(&connection)?;
    DB_CONNECTION
        .set(Mutex::new(connection))
        .map_err(|_| Error::other("setting oncelock failed"))?;
    Ok(())
}

fn create_schema(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(include_str!("sql/schema.sql"))
        .map_err(Error::other)
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
    let mut statement = connection.prepare_cached(&query).map_err(Error::other)?;
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
