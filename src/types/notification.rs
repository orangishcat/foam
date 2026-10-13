use crate::{database, state::notifications::NotificationState};
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
    pub is_unread: bool,
    pub event: NotificationEvent,
    pub title: String,
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    pub material_type: Option<MaterialType>,
    pub course_id: String,
    pub course_title: String,
}

impl Notification {
    pub fn id(&self) -> Result<String> {
        Ok(self.resource_id.clone())
    }
}

pub fn notifications() -> Result<Vec<Notification>> {
    database::from_sql(
        "SELECT * FROM notifications ORDER BY julianday(created) DESC, id".to_owned(),
        &[],
    )
}

pub fn save_notifications(
    notifications: &[Notification],
    notif_state: &NotificationState,
) -> Result<()> {
    let mut connection = database::connection()?;
    save_notifications_on(&mut connection, notifications, notif_state)
}

pub(crate) fn save_notifications_on(
    connection: &mut rusqlite::Connection,
    notifications: &[Notification],
    notif_state: &NotificationState,
) -> Result<()> {
    let transaction = connection.transaction().map_err(Error::other)?;
    for notification in notifications {
        let serialized = serde_rusqlite::to_params_named(notification).map_err(Error::other)?;
        let id = notification.id()?;
        let mut values = serialized.to_slice();
        values.push((":id", &id));
        transaction.execute("INSERT INTO notifications (id, is_unread, event, title, created, resource_id, material_type, course_id, course_title)
            VALUES (:id, :is_unread, :event, :title, :created, :resource_id, :material_type, :course_id, :course_title)
            ON CONFLICT(resource_id) DO UPDATE SET event = excluded.event, title = excluded.title, created = excluded.created, material_type = excluded.material_type, course_id = excluded.course_id, course_title = excluded.course_title
            WHERE julianday(excluded.created) >= julianday(notifications.created)", values.as_slice()).map_err(Error::other)?;
    }
    notif_state.save_state_on(&transaction)?;
    transaction.commit().map_err(Error::other)
}
