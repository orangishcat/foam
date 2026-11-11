use crate::{database, state::notifications::NotificationState};
use anyhow;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::resource::ResourceType;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub is_unread: bool,
    /// Opaque event type owned by the notification's source plugin.
    pub event: String,
    pub title: String,
    pub subtitle: String,
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    pub resource_type: Option<ResourceType>,
    // None when the notification is not scoped to one folder (source-global notes)
    pub folder_id: Option<String>,
    pub folder_title: String,
    /// 'builtin' | 'plugin:<id>' attribution; empty becomes builtin on write.
    pub source: String,
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
        transaction.execute("INSERT INTO notifications (id, is_unread, event, title, subtitle, created, resource_id, resource_type, folder_id, folder_title, source, seq)
            VALUES (:id, :is_unread, :event, :title, :subtitle, :created, :resource_id, :resource_type, :folder_id, :folder_title, COALESCE(NULLIF(:source, ''), 'builtin'), :seq)
            ON CONFLICT(resource_id) DO UPDATE SET event = excluded.event, title = excluded.title, subtitle = excluded.subtitle, created = excluded.created, resource_type = excluded.resource_type, folder_id = excluded.folder_id, folder_title = excluded.folder_title, source = excluded.source, seq = excluded.seq
            WHERE julianday(excluded.created) >= julianday(notifications.created)", values.as_slice())?;
    }
    notif_state.save_state_on(&transaction)?;
    Ok(transaction.commit()?)
}
