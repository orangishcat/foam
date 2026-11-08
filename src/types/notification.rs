use crate::{database, state::notifications::NotificationState};
use anyhow;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

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
    pub subtitle: String,
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    pub material_type: Option<MaterialType>,
    pub course_id: String,
    pub course_title: String,
    // scrape order within one sync; used as the display tiebreak for equal `created`
    pub seq: i64,
}

impl Notification {
    pub fn id(&self) -> anyhow::Result<String> {
        Ok(self.resource_id.clone())
    }
}

pub fn notifications() -> anyhow::Result<Vec<Notification>> {
    database::from_sql(
        "SELECT * FROM notifications ORDER BY julianday(created) DESC, seq".to_owned(),
        &[],
    )
}

pub fn save_notifications(
    notifications: &[Notification],
    notif_state: &NotificationState,
) -> anyhow::Result<()> {
    let mut connection = database::connection()?;
    save_notifications_on(&mut connection, notifications, notif_state)
}

pub(crate) fn save_notifications_on(
    connection: &mut rusqlite::Connection,
    notifications: &[Notification],
    notif_state: &NotificationState,
) -> anyhow::Result<()> {
    let transaction = connection.transaction()?;
    for notification in notifications {
        let serialized = serde_rusqlite::to_params_named(notification)?;
        let id = notification.id()?;
        let mut values = serialized.to_slice();
        values.push((":id", &id));
        transaction.execute("INSERT INTO notifications (id, is_unread, event, title, subtitle, created, resource_id, material_type, course_id, course_title, seq)
            VALUES (:id, :is_unread, :event, :title, :subtitle, :created, :resource_id, :material_type, :course_id, :course_title, :seq)
            ON CONFLICT(resource_id) DO UPDATE SET event = excluded.event, title = excluded.title, subtitle = excluded.subtitle, created = excluded.created, material_type = excluded.material_type, course_id = excluded.course_id, course_title = excluded.course_title, seq = excluded.seq
            WHERE julianday(excluded.created) >= julianday(notifications.created)", values.as_slice())?;
    }
    notif_state.save_state_on(&transaction)?;
    Ok(transaction.commit()?)
}
