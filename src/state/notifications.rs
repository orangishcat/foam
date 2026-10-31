use std::io::{self, Error, Result};

use chrono::{DateTime, Local};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow, UiState,
    config::config,
    database,
    state::state::state,
    thread_manager,
    types::{
        material::MaterialType,
        notification::{Notification, NotificationEvent},
    },
    ui,
};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct NotificationState {
    pub last_fast_sync: DateTime<Local>,
    pub last_slow_sync: DateTime<Local>,
    #[serde(skip)]
    is_fast_syncing: bool,
    #[serde(skip)]
    is_slow_syncing: bool,
    #[serde(skip, default = "default_notification_limit")]
    notification_limit: i32,
}

fn default_notification_limit() -> i32 {
    25
}

impl NotificationState {
    pub fn init(&mut self) {
        match Self::load() {
            Ok(saved) => {
                *self = saved;
                log::debug!(
                    "Last synced load: fast: {} slow: {}",
                    self.last_fast_sync,
                    self.last_slow_sync,
                );
            }
            Err(err) => log::warn!("Loading notification sync state failed: {err}"),
        }
    }

    pub fn check_notifications(&mut self) {
        if crate::account::active_account().is_err() {
            return;
        }
        if !self.is_fast_syncing && Local::now() - self.last_fast_sync >= config().refresh_duration
        {
            self.is_fast_syncing = true;
            log::debug!("Calling fast sync");
            if let Err(err) = thread_manager::spawn_thread("sync notifications", Self::fast_sync) {
                self.is_fast_syncing = false;
                log::warn!("Starting notification sync failed: {err}");
            }
        }
        if !self.is_slow_syncing
            && Local::now() - self.last_slow_sync >= config().submission_refresh_duration
        {
            log::info!("Calling slow sync");
            self.is_slow_syncing = true;
            if let Err(err) = thread_manager::spawn_thread("sync submissions", Self::slow_sync) {
                self.is_slow_syncing = false;
                log::warn!("Starting submission sync failed: {err}");
            }
        }
    }

    fn fast_sync() {
        let result = crate::account::active_account().and_then(|account| account.fast_sync());
        {
            let notif = &mut state().notif;
            notif.is_fast_syncing = false;
            notif.last_fast_sync = Local::now();
            match notif.save_state() {
                Ok(_) => log::debug!("Saved notification sync state"),
                Err(save_err) => {
                    log::warn!("Saving notification sync state failed: {save_err}")
                }
            }
            match result {
                Ok(_) => log::debug!("Notification sync success"),
                Err(err) => log::warn!("Notification sync failed: {err}"),
            }
        }
        ui::sync_ui();
    }

    fn slow_sync() {
        let result = crate::account::active_account().and_then(|account| account.slow_sync());
        {
            let notif = &mut state().notif;
            notif.is_slow_syncing = false;
            notif.last_slow_sync = Local::now();
            if let Err(err) = &result {
                log::warn!("Submission sync failed: {err}");
            }
        }
        ui::sync_ui();
    }

    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let notifications = database::from_sql_map(
            "SELECT n.*, COALESCE(c.course_title, NULLIF(n.course_title, ''), 'Unknown course') AS display_course
             FROM notifications n LEFT JOIN courses c ON n.course_id = c.course_id
             ORDER BY julianday(n.created) DESC, n.seq LIMIT ?1".to_owned(), &[&self.notification_limit], |row| {
                Ok((serde_rusqlite::from_row::<Notification>(row).map_err(Error::other)?,
                    row.get::<_, String>("display_course").map_err(Error::other)?))
            })?;
        let models = notifications
            .iter()
            .map(|(n, course)| {
                Ok(crate::Notification {
                    id: n.id()?.into(),
                    unread: n.is_unread,
                    title: (&n.title).into(),
                    subtitle: (&n.subtitle).into(),
                    course: course.into(),
                    course_id: (&n.course_id).into(),
                    resource_id: (&n.resource_id).into(),
                    n_type: match (n.event, n.material_type) {
                        (NotificationEvent::GradeUpdated, _) => crate::NotificationType::NewGrade,
                        (_, Some(MaterialType::Assignment | MaterialType::Assessment)) => {
                            crate::NotificationType::NewAssignment
                        }
                        (_, Some(MaterialType::Document)) => crate::NotificationType::NewDocument,
                        (_, Some(MaterialType::Link)) => crate::NotificationType::NewLink,
                        _ => crate::NotificationType::Unknown,
                    },
                    icon_color: match n.event {
                        NotificationEvent::GradeUpdated => {
                            match ui::progress_percent(&n.course_id, &n.resource_id) {
                                Some(percent) => ui::progress_color(ui, percent),
                                None => ui.global::<UiState>().get_theme().background_400,
                            }
                        }
                        _ => match n.material_type {
                            Some(MaterialType::Assignment | MaterialType::Assessment) => {
                                ui.global::<UiState>().get_theme().progress_900
                            }
                            _ => ui.global::<UiState>().get_theme().text_400,
                        },
                    },
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let global = ui.global::<crate::NotificationUi>();
        let selected_id = global.get_selected_notification().id;
        global.set_selected_notification(
            models
                .iter()
                .find(|n| n.id == selected_id)
                .cloned()
                .unwrap_or_default(),
        );
        global.set_unread_count(models.iter().filter(|n| n.unread).count() as i32);
        global.set_temp_notifs(ModelRc::new(VecModel::from(Vec::new())));
        global.set_notifications(ModelRc::new(VecModel::from(models)));
        Ok(())
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<crate::NotificationUi>();
        global.on_opened(|| {
            {
                let notif = &mut state().notif;
                notif.notification_limit = 20;
            }
            ui::sync_ui();
        });
        global.on_load_more(|| {
            {
                let notif = &mut state().notif;
                notif.notification_limit += 20;
            }
            ui::sync_ui();
        });
        global.on_closed(ui::sync_ui);
        global.on_mark_all_as_read(|| {
            if let Err(err) =
                database::execute("UPDATE notifications SET is_unread = 0".to_owned(), &[])
            {
                log::warn!("Marking notifications read failed: {err}");
            }
            ui::sync_ui();
        });
        global.on_set_read(|id, read| {
            if let Err(err) = database::execute(
                "UPDATE notifications SET is_unread = ? WHERE id = ?".to_owned(),
                &[&read, &id.as_str()],
            ) {
                log::warn!("Marking notification failed: {err}");
            }
            ui::sync_ui();
        });
    }

    pub fn sync_sidebar(&self, ui: &AppWindow) -> io::Result<()> {
        let notifications =
            database::from_sql::<Notification>("SELECT * FROM notifications".into(), &[])?;
        ui.global::<crate::NotificationUi>()
            .set_unread_count(notifications.iter().filter(|n| n.is_unread).count() as i32);
        Ok(())
    }

    pub fn save_state(&self) -> Result<()> {
        let connection = database::connection()?;
        self.save_state_on(&connection)
    }

    pub(crate) fn save_state_on(&self, connection: &Connection) -> Result<()> {
        connection.execute("INSERT INTO sync_state (key, data) VALUES ('notifications', ?) ON CONFLICT(key) DO UPDATE SET data = excluded.data",
            params![serde_json::to_string(self).map_err(Error::other)?]).map_err(Error::other)?;
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let values = database::from_sql_map(
            "SELECT data FROM sync_state WHERE key = 'notifications'".to_owned(),
            &[],
            |row| {
                let data: String = row.get(0).map_err(Error::other)?;
                serde_json::from_str(&data).map_err(Error::other)
            },
        )?;
        Ok(values.into_iter().next().unwrap_or_default())
    }
}
