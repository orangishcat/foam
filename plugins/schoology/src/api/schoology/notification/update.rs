use crate::api::schoology::account::{ResetProgress, SchoologyAccountConfig};
use foam::ui;

const NOTIFICATION_PROGRESS_PRIORITY: u8 = 1;
const NOTIFICATION_PROGRESS_COLOR: slint::Color = slint::Color::from_rgb_u8(0x78, 0xaa, 0xff);
use crate::api::schoology::RequestResult;
use foam::{
    database,
    thread_manager::check_cancelled,
    types::{
        folder_info::FolderInfo,
        notification::{Notification, NotificationEvent},
    },
};
use std::{collections::HashSet, io};

impl SchoologyAccountConfig {
    /// Process each folder hierarchy once. Failed items remain pending.
    pub fn update_notifications(&self, notifications: &mut [Notification]) -> RequestResult<()> {
        let _reset = ResetProgress(NOTIFICATION_PROGRESS_PRIORITY, NOTIFICATION_PROGRESS_COLOR);
        let mut refreshed = HashSet::new();
        let total = notifications.len();
        let mut failure = None;
        let mut attempted = 0;
        ui::publish_progress(
            1.0,
            NOTIFICATION_PROGRESS_PRIORITY,
            &NOTIFICATION_PROGRESS_COLOR,
        );
        for i in 0..notifications.len() {
            // Grade notifications are processed as one batch below.
            if notifications[i].event == NotificationEvent::GradeUpdated {
                continue;
            }
            check_cancelled()?;
            let posted = notifications
                .iter()
                .filter(|n| {
                    n.event == NotificationEvent::ResourcePosted
                        && n.folder_id == notifications[i].folder_id
                })
                .map(|n| n.resource_id.clone())
                .collect();
            if let Err(err) =
                self.process_notification(&mut notifications[i], &posted, &mut refreshed)
            {
                log::warn!(
                    "Updating notification {} failed: {err}",
                    notifications[i].resource_id
                );
                failure = Some(err);
            }
            attempted += 1;
            ui::publish_progress(
                attempted as f32 / total as f32,
                NOTIFICATION_PROGRESS_PRIORITY,
                &NOTIFICATION_PROGRESS_COLOR,
            );
        }
        ui::publish_progress(0.0, NOTIFICATION_PROGRESS_PRIORITY, &NOTIFICATION_PROGRESS_COLOR);
        // One unfiltered request for the entire grade-notification batch,
        // including on failure — same shape as the pre-split code, but the
        // grades land in the plugin's own table and the messages carry the
        // task-detail-era grade text.
        if notifications
            .iter()
            .any(|n| n.event == NotificationEvent::GradeUpdated)
        {
            check_cancelled()?;
            match self.fetch_grades().and_then(|grades| Ok(self.apply_grades(&grades)?)) {
                Ok(()) => {
                    for n in notifications
                        .iter_mut()
                        .filter(|n| n.event == NotificationEvent::GradeUpdated)
                    {
                        if let Some(folder_id) = n.folder_id.as_deref().filter(|id| !id.is_empty()) {
                            if let Ok(subtitle) = self.grade_subtitle(folder_id, &n.resource_id) {
                                n.subtitle = subtitle;
                            }
                        }
                    }
                }
                Err(err) => {
                    log::warn!("Updating notification grades failed: {err}");
                    failure.get_or_insert(err);
                }
            }
        }
        match failure {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn resolve_notification_folder(&self, n: &Notification) -> RequestResult<FolderInfo> {
        let Some(folder_id) = n.folder_id.as_deref().filter(|id| !id.is_empty()) else {
            return Err(io::Error::other("notification folder ID is empty").into());
        };
        if let Some(folder) = foam::types::folder_info::folder(folder_id)? {
            return Ok(folder);
        }
        // fetch without holding the state lock; schoology section ids are sometimes incorrect for some reason
        let title = self.section_folder_title(folder_id)?;
        Ok(foam::types::folder_info::resolve_folder_alias(
            folder_id,
            &title,
        )?)
    }

    fn process_notification(
        &self,
        n: &mut Notification,
        posted: &HashSet<String>,
        refreshed: &mut HashSet<String>,
    ) -> RequestResult<()> {
        if n.resource_id.is_empty() {
            return Err(io::Error::other("notification resource ID is empty").into());
        }

        match n.event {
            NotificationEvent::GradeUpdated => {} // handled in plugin storage
            NotificationEvent::ResourcePosted => {
                let folder = self.resolve_notification_folder(n)?;
                if !refreshed.contains(&folder.folder_id) {
                    let cached = foam::types::resource::resources(&folder.folder_id)?;
                    let hierarchy =
                        self.hierarchy(&folder.folder_id, &cached, posted)?;
                    check_cancelled()?;
                    foam::types::resource::store_hierarchy(&folder.folder_id, &hierarchy)?;
                    refreshed.insert(folder.folder_id.clone());
                }
                let kinds = database::from_sql_map(
                    "SELECT type FROM resources WHERE folder_id = ? AND resource_id = ?".to_owned(),
                    &[&folder.folder_id, &n.resource_id],
                    |row| {
                        let kind: String = row.get(0)?;
                        Ok(serde_json::from_value(serde_json::Value::String(kind))?)
                    },
                )?;
                let kind = kinds.into_iter().next().ok_or_else(|| {
                    io::Error::other("posted resource was not loaded from hierarchy")
                })?;
                n.resource_type = Some(kind);
                n.folder_id = Some(folder.folder_id.clone());
                n.folder_title = folder.title.clone();
            }
            NotificationEvent::Unknown => {
                return Err(io::Error::other("unsupported notification event").into());
            }
        }
        Ok(())
    }
}
