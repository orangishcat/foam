use chrono::{DateTime, Local};
use slint::ComponentHandle;

use crate::{
    AppWindow,
    api::schoology,
    config::{config, config_write},
    state::{courses::CourseState, state::state},
    thread_manager,
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
                state().notifs.notifications = notifs.clone();
                config_write().last_update = Local::now();
                log::info!("Finished scraping notifications");
                if let Err(err) = Self::update_notif_materials(notifs, Local::now()) {
                    state().notifs.is_checking_notifications = false;
                    log::warn!("Starting notification material update failed: {err}");
                }
            } else {
                state().notifs.is_checking_notifications = false;
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
            let mut courses: Vec<_> = state().courses.courses.values().cloned().collect();
            let publish_progress = |progress| {
                crate::ui::run_on_ui_thread(move |ui| {
                    let global = ui.global::<crate::UiState>();
                    let mut notif = global.get_notif();
                    notif.progress = progress;
                    global.set_notif(notif);
                })
            };
            publish_progress(0.0);
            let result = schoology::notification::update::update(
                &mut notifs,
                &mut courses,
                publish_progress,
            );
            let persisted = crate::filesystem::write_courses(&courses);
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
    pub fn sync_ui(&self, _courses: &CourseState, _ui: &AppWindow) {}
}
