use chrono::{DateTime, Local};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow,
    api::schoology,
    config::{config, config_write},
    state::{courses::CourseState, state::state},
    thread_manager::{self, check_cancelled},
    types::notification::Notification,
};

#[derive(Default)]
pub struct NotificationState {
    pub notifications: Vec<Notification>,
    is_checking_notifications: bool,
}

impl NotificationState {
    pub fn check_notifications(&mut self) {
        if Local::now() - config().last_update < config().refresh_duration
            || self.is_checking_notifications
        {
            log::debug!("Notifications cache is fresh, skipping fetch");
            return;
        }

        self.is_checking_notifications = true;

        if let Err(err) = Self::spawn_notif_scrape() {
            self.is_checking_notifications = false;
            log::warn!("Scraping notifications failed: {err}");
        }
    }

    fn spawn_notif_scrape() -> std::io::Result<()> {
        thread_manager::spawn_thread("scrape notifications", || {
            if let Ok(mut notifs) = schoology::notification::scrape_notifications() {
                let last_sync = config().last_sync;
                for notif in notifs.iter_mut() {
                    notif.is_processed = notif.created < last_sync;
                }
                state().notif.notifications = notifs.clone();
                config_write().last_update = Local::now();
                log::info!("Finished scraping notifications");
                if let Err(_) = check_cancelled() {
                    return;
                }
                if let Err(err) = Self::update_notif_materials(notifs, Local::now()) {
                    state().notif.is_checking_notifications = false;
                    log::warn!("Starting notification material update failed: {err}");
                }
            } else {
                state().notif.is_checking_notifications = false;
                log::warn!("Scraping notifications failed");
            }
        })
        .map(|_| ())
    }

    fn update_notif_materials(
        mut notifs: Vec<Notification>,
        check_started: DateTime<Local>,
    ) -> std::io::Result<()> {
        thread_manager::spawn_thread("update notification materials", move || {
            let publish_progress = |progress| {
                crate::ui::run_on_ui_thread(move |ui| {
                    let global = ui.global::<crate::UiState>();
                    let mut notif = global.get_notif();
                    notif.progress = progress;
                    global.set_notif(notif);
                })
            };
            publish_progress(1.0);
            let result = schoology::notification::update::update(&mut notifs, publish_progress);
            let persisted = state().course.save();
            if let Err(err) = &result {
                log::warn!("Updating notification materials failed: {err}");
            }
            if let Err(err) = &persisted {
                log::warn!("Saving notification materials failed: {err}");
            }
            if result.is_ok() && persisted.is_ok() {
                // notifications arriving during sync are marked as not synced
                // the logic is here so that last_sync is only updated when sync is successful
                config_write().last_sync = check_started;
            }
            crate::ui::run_on_ui_thread(move |ui| {
                state().sync_ui(&ui);
            });
        })
        .map(|_| ())
    }
    pub fn sync_ui(&self, courses: &CourseState, ui: &AppWindow) {
        let notification_models = self
            .notifications
            .iter()
            .map(|notif| crate::Notification {
                title: notif.title.to_owned().into(),
                course: courses
                    .get_course(&notif.course_id)
                    .map_or("Unknown course", |c| c.course_title.as_str())
                    .into(),
                course_id: notif.course_id.to_owned().into(),
            })
            .take(20)
            .collect::<Vec<crate::Notification>>();
        ui.global::<crate::UiState>()
            .set_notif(crate::NotificationUi {
                progress: 0.0,
                temp_notifs: ModelRc::new(VecModel::from(Vec::new())), // todo!
                notifications: ModelRc::new(VecModel::from(notification_models)),
            });
    }
}
