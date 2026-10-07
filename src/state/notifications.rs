use std::{
    cmp::min,
    io::{self, Error, Result},
};

use chrono::{DateTime, Local, TimeDelta};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow, UiState, api,
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
    #[serde(default)]
    pub last_opened: DateTime<Local>,
    #[serde(skip)]
    visit_cutoff: Option<DateTime<Local>>,
    pub last_update: DateTime<Local>,
    pub last_sync: DateTime<Local>,
    pub last_submission_sync: DateTime<Local>,
    #[serde(skip)]
    pub(crate) last_update_success: bool,
    #[serde(skip)]
    pub(crate) scrape_attempts: u32,
    #[serde(skip)]
    is_checking_notifications: bool,
    #[serde(skip)]
    is_slow_syncing: bool,
}

impl NotificationState {
    pub fn init(&mut self) {
        match load_sync_state() {
            Ok(saved) => *self = saved,
            Err(err) => log::warn!("Loading notification sync state failed: {err}"),
        }
    }

    pub fn check_notifications(&mut self) {
        if !self.is_checking_notifications
            && Local::now() - self.last_update >= self.refresh_duration()
        {
            self.is_checking_notifications = true;
            self.scrape_attempts += 1;
            if let Err(err) = thread_manager::spawn_thread("sync notifications", Self::fast_sync) {
                self.is_checking_notifications = false;
                log::warn!("Starting notification sync failed: {err}");
            }
        }
        // Submission sync can run alongside notifications: each updates only its own SQL columns.
        if !self.is_slow_syncing
            && Local::now() - self.last_submission_sync >= config().submission_refresh_duration
        {
            self.is_slow_syncing = true;
            if let Err(err) = thread_manager::spawn_thread("sync submissions", Self::slow_sync) {
                self.is_slow_syncing = false;
                log::warn!("Starting submission sync failed: {err}");
            }
        }
    }

    fn refresh_duration(&self) -> TimeDelta {
        if self.last_update_success {
            config().refresh_duration
        } else {
            min(
                api::exponential_retry(self.scrape_attempts),
                config().refresh_duration,
            )
        }
    }

    fn fast_sync() {
        let result = crate::account::active_account().and_then(|account| account.fast_sync());
        {
            let mut app = state();
            app.notif.is_checking_notifications = false;
            if let Err(err) = result {
                app.notif.last_update_success = false;
                app.notif.last_update = Local::now();
                if let Err(save_err) = save_sync_state(&app.notif) {
                    log::warn!("Saving notification sync state failed: {save_err}");
                }
                log::warn!("Notification sync failed: {err}");
            }
        }
        ui::sync_ui();
    }

    fn slow_sync() {
        let result = crate::account::active_account().and_then(|account| account.slow_sync());
        state().notif.is_slow_syncing = false;
        if let Err(err) = result {
            log::warn!("Submission sync failed: {err}");
        }
        ui::sync_ui();
    }

    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        let cutoff = if ui.global::<UiState>().get_screen() == crate::Screen::Notifs {
            self.visit_cutoff.unwrap_or(self.last_opened)
        } else {
            self.last_opened
        };
        let notifications = database::from_sql_map(
            "SELECT n.*, COALESCE(c.course_title, NULLIF(n.course_title, ''), 'Unknown course') AS display_course
             FROM notifications n LEFT JOIN courses c ON n.course_id = c.course_id
             ORDER BY julianday(n.created) DESC, n.id".to_owned(), &[], |row| {
                Ok((serde_rusqlite::from_row::<Notification>(row).map_err(Error::other)?,
                    row.get::<_, String>("display_course").map_err(Error::other)?))
            })?;
        let models = notifications
            .into_iter()
            .map(|(n, course)| {
                Ok(crate::Notification {
                    id: n.id()?.into(),
                    unread: n.is_unread(cutoff),
                    title: n.title.into(),
                    course: course.into(),
                    course_id: n.course_id.into(),
                    resource_id: n.resource_id.into(),
                    n_type: match (n.event, n.material_type) {
                        (NotificationEvent::GradeUpdated, _) => crate::NotificationType::NewGrade,
                        (_, Some(MaterialType::Assignment | MaterialType::Assessment)) => {
                            crate::NotificationType::NewAssignment
                        }
                        (_, Some(MaterialType::Document)) => crate::NotificationType::NewDocument,
                        (_, Some(MaterialType::Link)) => crate::NotificationType::NewLink,
                        _ => crate::NotificationType::Unknown,
                    },
                    icon_color: match n.material_type {
                        Some(MaterialType::Assignment | MaterialType::Assessment) => {
                            ui.global::<UiState>().get_theme().accent_400
                        }
                        _ => ui.global::<UiState>().get_theme().text_400,
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
        global.set_progress(0.0);
        global.set_temp_notifs(ModelRc::new(VecModel::from(Vec::new())));
        global.set_notifications(ModelRc::new(VecModel::from(models)));
        Ok(())
    }

    pub fn init_ui(&self, ui: &AppWindow) {
        let global = ui.global::<crate::NotificationUi>();
        global.on_closed(ui::sync_ui);
        global.on_opened(|| {
            let mut app = state();
            let mut next = app.notif.clone();
            next.visit_cutoff = Some(next.last_opened);
            next.last_opened = Local::now();
            match save_sync_state(&next) {
                Ok(()) => app.notif = next,
                Err(err) => log::warn!("Saving notification opened time failed: {err}"),
            }
            drop(app);
            ui::sync_ui();
        });
        global.on_set_read(|id, read| {
            if let Err(err) = database::execute(
                "UPDATE notifications SET manual_mark = ? WHERE id = ?".to_owned(),
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
        ui.global::<crate::NotificationUi>().set_unread_count(
            notifications
                .iter()
                .filter(|n| n.is_unread(self.last_opened))
                .count() as i32,
        );
        Ok(())
    }

    pub(crate) fn publish_progress(progress: f32) {
        ui::run_on_ui_thread(move |ui| {
            ui.global::<crate::NotificationUi>().set_progress(progress);
        });
    }
}

pub fn load_sync_state<T: DeserializeOwned + Default>() -> Result<T> {
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

pub fn save_sync_state(state: &impl Serialize) -> Result<()> {
    let connection = database::connection()?;
    save_sync_state_on(&connection, state)
}

pub(crate) fn save_sync_state_on(connection: &Connection, state: &impl Serialize) -> Result<()> {
    connection.execute("INSERT INTO sync_state (key, data) VALUES ('notifications', ?) ON CONFLICT(key) DO UPDATE SET data = excluded.data",
        params![serde_json::to_string(state).map_err(Error::other)?]).map_err(Error::other)?;
    Ok(())
}
