use crate::{database, state::notifications::save_sync_state_on};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::io::{Error, Result};

use super::material::MaterialType;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    MaterialPosted,
    GradeUpdated,
    #[default]
    Unknown,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub event: NotificationEvent,
    pub title: String,
    pub viewed: bool,
    pub is_processed: bool, // whether foam has processed this notification by updating syncing
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    pub material_type: Option<MaterialType>,
    pub course_id: String,
    pub course_title: String,
}

pub fn notifications() -> Result<Vec<Notification>> {
    database::from_sql(
        "SELECT * FROM notifications ORDER BY julianday(created) DESC, id".to_owned(),
        &[],
    )
}

pub fn save_notifications(notifications: &[Notification], state: &impl Serialize) -> Result<()> {
    let mut connection = database::connection()?;
    save_notifications_on(&mut connection, notifications, state)
}

pub(crate) fn save_notifications_on(
    connection: &mut rusqlite::Connection,
    notifications: &[Notification],
    state: &impl Serialize,
) -> Result<()> {
    let transaction = connection.transaction().map_err(Error::other)?;
    transaction
        .execute("DELETE FROM notifications", [])
        .map_err(Error::other)?;
    for notification in notifications {
        let serialized = serde_rusqlite::to_params_named(notification).map_err(Error::other)?;
        let id = serde_json::to_string(&(
            notification.event,
            &notification.resource_id,
            notification.created,
        ))
        .map_err(Error::other)?;
        let mut values = serialized.to_slice();
        values.push((":id", &id));
        transaction.execute("INSERT INTO notifications (id, event, title, viewed, is_processed, created, resource_id, material_type, course_id, course_title)
            VALUES (:id, :event, :title, :viewed, :is_processed, :created, :resource_id, :material_type, :course_id, :course_title)
            ON CONFLICT(id) DO UPDATE SET viewed = excluded.viewed, is_processed = excluded.is_processed, title = excluded.title, material_type = excluded.material_type, course_id = excluded.course_id, course_title = excluded.course_title", values.as_slice()).map_err(Error::other)?;
    }
    save_sync_state_on(&transaction, state)?;
    transaction.commit().map_err(Error::other)
}
