use std::{
    io,
    sync::{Arc, OnceLock},
};

use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};

use crate::{
    account::{Account, RequestResult},
    api::schoology::cookies,
    config::config,
    database, filesystem,
    state::state::state,
    thread_manager::check_cancelled,
    types::{assignment::Assignment, notification, submission},
    ui,
};

pub(super) struct ResetProgress(pub(super) u8, pub(super) slint::Color);

impl Drop for ResetProgress {
    fn drop(&mut self) {
        ui::publish_progress(0.0, self.0, &self.1);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SchoologyAccountConfig {
    pub subdomain: String,
    pub user_id: String,
    pub calendar_url: String,
    pub cookie_key: String,
    pub cookie_value: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    #[serde(skip)]
    pub(super) api_client: Arc<OnceLock<RequestResult<reqwest::blocking::Client>>>,
    #[serde(skip)]
    pub(super) internal_client: Arc<OnceLock<RequestResult<reqwest::blocking::Client>>>,
    #[serde(skip)]
    pub(super) cookies: Arc<OnceLock<RequestResult<Arc<cookies::SessionCookies>>>>,
}

impl Account for SchoologyAccountConfig {
    fn clone_account(&self) -> Box<dyn Account> {
        Box::new(self.clone())
    }

    fn assignment_url(&self, id: &str) -> String {
        format!(
            "https://{}.schoology.com/assignment/{id}",
            self.subdomain.trim()
        )
    }

    fn resource_headers(&self, url: &str) -> RequestResult<reqwest::header::HeaderMap> {
        Self::resource_headers(self, url)
    }

    fn load() -> Result<Self, std::io::Error> {
        // todo: supply the data dir
        filesystem::read_json(&config().data_dir().join("plugins").join("schoology.json"))
    }

    fn save(&self) -> Result<(), std::io::Error> {
        filesystem::write_json(
            &config().data_dir().join("plugins").join("schoology.json"),
            self,
        )
    }

    fn get_resource(&self, url: &str) -> RequestResult<reqwest::blocking::RequestBuilder> {
        let request = if url.starts_with("https://api.schoology.com") {
            self.api_get_request(url)?
        } else {
            self.internal_get_request(url)?
        };
        Ok(request.header(ACCEPT, "*/*"))
    }

    fn fast_sync(&self) -> io::Result<()> {
        let check_started = chrono::Local::now();
        let loaded = database::from_sql_map(
            "SELECT data FROM sync_state WHERE key = 'courses_loaded'".to_owned(),
            &[],
            |row| row.get::<_, String>(0).map_err(io::Error::other),
        )?;
        if loaded.is_empty() {
            self.scrape_courses().map_err(io::Error::other)?;
            database::execute("INSERT INTO sync_state (key, data) VALUES ('courses_loaded', 'true') ON CONFLICT(key) DO NOTHING".to_owned(), &[])?;
        }
        let mut notifications = self.scrape_notifications().map_err(io::Error::other)?;
        let previous = notification::notifications()?;
        let last_sync = state().notif.last_sync;
        notifications.sort_by_key(|n| std::cmp::Reverse(n.created));
        let mut seen = std::collections::HashSet::new();
        notifications.retain(|n| seen.insert(n.resource_id.clone()));
        let mut pending: Vec<_> = notifications
            .iter()
            .filter(|n| {
                n.created >= last_sync
                    || !previous.iter().any(|old| old.resource_id == n.resource_id)
            })
            .cloned()
            .collect();
        check_cancelled()?;
        let update_result = self.update_notifications(&mut pending);
        for updated in pending {
            if let Some(n) = notifications
                .iter_mut()
                .find(|n| n.title == updated.title || n.resource_id == updated.resource_id)
            {
                *n = updated;
            }
        }
        match self
            .fetch_calendar()
            .and_then(|(ids, assignments)| Ok(self.apply_calendar(&ids, &assignments)?))
        {
            Ok(updated) => log::debug!("Updated {updated} calendar assignments"),
            Err(err) => log::warn!("Calendar sync failed: {err}"),
        }
        let next = &mut state().notif;
        if update_result.is_ok() {
            next.last_sync = check_started;
        }
        next.last_update = chrono::Local::now();
        next.last_update_success = update_result.is_ok();
        if next.last_update_success {
            next.scrape_attempts = 0;
        }
        notification::save_notifications(&notifications, next)?;
        update_result.map_err(io::Error::other)
    }

    fn slow_sync(&self) -> io::Result<()> {
        let check_started = chrono::Local::now();
        (|| -> RequestResult<()> {
            let loaded = database::from_sql_map(
                "SELECT data FROM sync_state WHERE key = 'courses_loaded'".to_owned(),
                &[],
                |row| row.get::<_, String>(0).map_err(io::Error::other),
            )?;
            if loaded.is_empty() {
                return Ok(());
            }
            let mut assignments = database::from_sql::<Assignment>(
                format!(
                    "SELECT * FROM assignments WHERE NOT ({})",
                    include_str!("../../sql/assignment/completed.sql")
                ),
                &[],
            )?;
            self.scrape_submissions(&mut assignments)?;
            check_cancelled()?;
            submission::update_submissions(&assignments)?;
            let mut app = state();
            app.notif.last_submission_sync = check_started;
            app.notif.save_state()?;
            Ok(())
        })()
        .map_err(io::Error::other)
    }
}
