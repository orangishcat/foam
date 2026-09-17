use crate::database;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_with::{PickFirst, Same, json::JsonString, serde_as};
use std::io::{Error, Result};

use super::folder::Folder;

#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Course {
    pub course_id: String, // in schoology this is actually the section id
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub aliases: Vec<String>,
    pub course_title: String,
    pub course_code: String,
    pub course_url: String,
    pub section_title: String,
    pub section_code: String,

    pub active: bool,
    pub description: String,
    pub logo_img_src: String,
    pub location: String,
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub meeting_days: Vec<i8>,
    pub start_time: String,
    pub end_time: String,
    pub weight: String,
}

pub fn course(id: &str) -> Result<Option<Course>> {
    let connection = database::connection()?;
    course_from(&connection, id)
}

pub(crate) fn course_from(connection: &Connection, id: &str) -> Result<Option<Course>> {
    // Canonical IDs win over aliases. Ambiguous aliases must not select an arbitrary course.
    let exact = database::read_from::<Course>(
        connection,
        "SELECT * FROM courses WHERE course_id = ?",
        params![id],
    )?;
    if let Some(course) = exact.into_iter().next() {
        return Ok(Some(course));
    }
    let mut matches = database::read_from::<Course>(
        connection,
        "SELECT * FROM courses WHERE EXISTS (SELECT 1 FROM json_each(courses.aliases) WHERE value = ?)",
        params![id],
    )?;
    if matches.len() > 1 {
        return Err(Error::other("ambiguous course alias"));
    }
    Ok(matches.pop())
}

pub fn resolve_course_alias(id: &str, title: &str) -> Result<Course> {
    let mut connection = database::connection()?;
    resolve_alias_on(&mut connection, id, title)
}

pub(crate) fn resolve_alias_on(
    connection: &mut Connection,
    id: &str,
    title: &str,
) -> Result<Course> {
    let transaction = connection.transaction().map_err(Error::other)?;
    if let Some(course) = course_from(&transaction, id)? {
        return Ok(course);
    }
    let mut matches = database::read_from::<Course>(
        &transaction,
        "SELECT * FROM courses WHERE course_title = ? AND ? != ''",
        params![title, title],
    )?;
    if matches.len() != 1 {
        return Err(Error::other(
            "notification course title is missing or ambiguous",
        ));
    }
    let mut course = matches.pop().unwrap();
    course.aliases.push(id.to_owned());
    write_course_on(&transaction, &course)?;
    transaction.commit().map_err(Error::other)?;
    Ok(course)
}

pub(crate) fn write_course_on(connection: &Connection, course: &Course) -> Result<()> {
    let serialized = serde_rusqlite::to_params_named(course).map_err(Error::other)?;
    connection
        .execute(
            include_str!("../sql/course/write.sql"),
            serialized.to_slice().as_slice(),
        )
        .map_err(Error::other)?;
    Ok(())
}

/// Persist a fetched course and its hierarchy atomically. Existing assignment state wins.
pub fn store_course(course: &Course, root: &Folder) -> Result<()> {
    let mut connection = database::connection()?;
    let transaction = connection.transaction().map_err(Error::other)?;
    let mut course = course.clone();
    if let Some(current) = database::read_from::<Course>(
        &transaction,
        "SELECT * FROM courses WHERE course_id = ?",
        params![course.course_id],
    )?
    .into_iter()
    .next()
    {
        course.aliases = current.aliases;
    }
    write_course_on(&transaction, &course)?;
    super::material::store_hierarchy_on(&transaction, &course.course_id, root)?;
    transaction.commit().map_err(Error::other)
}
