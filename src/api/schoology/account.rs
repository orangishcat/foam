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
    state::{
        notifications::{NotificationState, save_sync_state},
        state::state,
    },
    thread_manager::check_cancelled,
    types::{assignment::Assignment, notification, submission},
};

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
        (|| -> RequestResult<()> {
            let loaded = database::from_sql_map(
                "SELECT data FROM sync_state WHERE key = 'courses_loaded'".to_owned(),
                &[],
                |row| row.get::<_, String>(0).map_err(io::Error::other),
            )?;
            if loaded.is_empty() {
                self.scrape_courses()?;
                database::execute("INSERT INTO sync_state (key, data) VALUES ('courses_loaded', 'true') ON CONFLICT(key) DO NOTHING".to_owned(), &[])?;
            }
            let mut notifications = self.scrape_notifications()?;
            let previous = notification::notifications()?;
            let last_sync = state().notif.last_sync;
            for n in &mut notifications {
                n.is_processed = previous
                    .iter()
                    .find(|old| {
                        old.event == n.event
                            && old.resource_id == n.resource_id
                            && old.created == n.created
                    })
                    .map_or(n.created < last_sync, |old| old.is_processed);
            }
            check_cancelled()?;
            let update_result = self.update_notifications(&mut notifications, &mut NotificationState::publish_progress);
            match self.fetch_calendar().and_then(|(ids, assignments)| {
                Ok(self.apply_calendar(&ids, &assignments, &mut NotificationState::publish_progress)?)
            }) {
                Ok(updated) => log::debug!("Updated {updated} calendar assignments"),
                Err(err) => log::warn!("Calendar sync failed: {err}"),
            }
            let mut app = state();
            let mut next = app.notif.clone();
            if update_result.is_ok() {
                next.last_sync = check_started;
            }
            next.last_update = chrono::Local::now();
            next.last_update_success =
                update_result.is_ok() && notifications.iter().all(|n| n.is_processed);
            if next.last_update_success {
                next.scrape_attempts = 0;
            }
            notification::save_notifications(&notifications, &next)?;
            app.notif = next;
            update_result
        })().map_err(io::Error::other)
    }

    fn slow_sync(&self) -> io::Result<()> {
        let check_started = chrono::Local::now();
        (|| -> RequestResult<()> {
            // Wait for the fast sync to populate the initial course cache.
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
            let mut next = app.notif.clone();
            next.last_submission_sync = check_started;
            save_sync_state(&next)?;
            app.notif = next;
            Ok(())
        })()
        .map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_account_snapshot_shares_clients_and_credentials() {
        let account = SchoologyAccountConfig {
            subdomain: "example".into(),
            cookie_key: "session".into(),
            cookie_value: "test-cookie".into(),
            ..Default::default()
        };
        let snapshot = account.clone_account();
        std::thread::spawn(move || {
            assert_eq!(
                snapshot.assignment_url("123"),
                "https://example.schoology.com/assignment/123"
            );
            let headers = snapshot
                .resource_headers("https://example.schoology.com/file")
                .unwrap();
            assert_eq!(headers[reqwest::header::COOKIE], "session=test-cookie");
            snapshot
                .get_resource("https://example.schoology.com/file")
                .unwrap()
                .build()
                .unwrap();
        })
        .join()
        .unwrap();
        assert!(account.internal_client.get().is_some());
        assert!(account.cookies.get().is_some());
    }
}
