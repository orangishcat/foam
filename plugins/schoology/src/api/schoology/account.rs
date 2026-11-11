use std::sync::{Arc, OnceLock};

use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};

use crate::api::schoology::cookies;
use anyhow;
use foam::{
    account::{Account, RequestResult},
    config::config,
    database,
    filesystem,
    state::state::state,
    thread_manager::check_cancelled,
    types::notification,
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

    fn task_url(&self, id: &str) -> String {
        format!(
            "https://{}.schoology.com/assignment/{id}",
            self.subdomain.trim()
        )
    }

    fn resource_headers(&self, url: &str) -> RequestResult<reqwest::header::HeaderMap> {
        Self::resource_headers(self, url)
    }

    fn load() -> anyhow::Result<Self> {
        // todo: supply the data dir
        filesystem::read_json(&config().data_dir().join("plugins").join("schoology.json"))
    }

    fn save(&self) -> anyhow::Result<()> {
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

    fn fast_sync(&self) -> anyhow::Result<()> {
        // The plugin authorizer scope: every DB write below (folders, tasks,
        // resources, notifications, sync_state, own plugin tables) follows the
        // same policy wasm plugins get from the host fns.
        let _scope = foam::plugin::scoped(super::plugin_data::PLUGIN_ID);
        // Force a fresh full scrape when the scraped layout version changed
        // (one-turn dev step in place of a real migration).
        if let Err(err) = super::plugin_data::ensure_scrape_version() {
            log::warn!("Checking scrape layout version failed: {err}");
        }
        let loaded = database::from_sql_map(
            "SELECT data FROM sync_state WHERE key = 'folders_loaded'".to_owned(),
            &[],
            |row| Ok(row.get::<_, String>(0)?),
        )?;
        if loaded.is_empty() {
            self.scrape_folders()
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            database::execute("INSERT INTO sync_state (key, data) VALUES ('folders_loaded', 'true') ON CONFLICT(key) DO NOTHING".to_owned(), &[])?;
        }
        let mut scraped_notifs = self
            .scrape_notifications()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let existing_notifs = notification::notifications()?;
        let last_sync = state().notif.last_fast_sync;
        scraped_notifs.sort_by_key(|n| n.created);
        let mut seen = std::collections::HashSet::new();
        scraped_notifs.retain(|n| seen.insert((n.resource_id.clone(), n.title.clone())));
        let mut pending: Vec<_> = scraped_notifs
            .iter()
            .filter(|n| {
                n.created >= last_sync
                    || !existing_notifs
                        .iter()
                        .any(|old| old.resource_id == n.resource_id)
            })
            .cloned()
            .collect();
        check_cancelled()?;
        let update_result = self.update_notifications(&mut pending);
        for updated in pending {
            if let Some(n) = scraped_notifs
                .iter_mut()
                .find(|n| n.title == updated.title || n.resource_id == updated.resource_id)
            {
                *n = updated;
            }
        }
        match self
            .fetch_calendar()
            .and_then(|(ids, cal_tasks)| Ok(self.apply_calendar(&ids, &cal_tasks)?))
        {
            Ok(updated) => log::debug!("Updated {updated} calendar tasks"),
            Err(err) => log::warn!("Calendar sync failed: {err}"),
        }
        // clone to avoid deadlock
        let next = state().notif.clone();
        notification::save_notifications(&scraped_notifs, &next)?;
        update_result.map_err(|error| anyhow::anyhow!("{error}"))
    }

    fn slow_sync(&self) -> anyhow::Result<()> {
        let check_started = chrono::Local::now();
        // Submissions/grades live in the Schoology plugin's own storage; core
        // tasks carry only generic fields. Same scoped-authorizer policy as the
        // fast sync (see above).
        let _scope = foam::plugin::scoped(super::plugin_data::PLUGIN_ID);
        (|| -> anyhow::Result<()> {
            super::plugin_data::ensure_table()?;
            let loaded = database::from_sql_map(
                "SELECT data FROM sync_state WHERE key = 'folders_loaded'".to_owned(),
                &[],
                |row| Ok(row.get::<_, String>(0)?),
            )?;
            if loaded.is_empty() {
                return Ok(());
            }
            // Only scrape submissions for tasks the old `completed` predicate
            // considered incomplete: unmarked, unscored, not submitted and
            // (past due but still accepting submissions does not count).
            let incomplete: Vec<(String, String)> = database::from_sql_map(
                "SELECT t.folder_id, t.id FROM tasks t
                 LEFT JOIN plugin_schoology_assignments a ON a.folder_id = t.folder_id AND a.id = t.id
                 WHERE a.id IS NULL OR (
                     a.manual_mark IS NULL
                     AND a.score IS NULL
                     AND a.letter_grade IS NULL
                     AND a.submissions = '[]'
                     AND (t.due IS NULL OR julianday(t.due) >= julianday('now') OR COALESCE(a.allow_submissions, 1) = 1)
                 )"
                .to_owned(),
                &[],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            if !incomplete.is_empty() {
                self.scrape_submissions(incomplete)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            }
            check_cancelled()?;
            Ok(())
        })()
        .inspect_err(|error| log::warn!("Schoology plugin sync failed: {error}"))
        .ok();
        let mut app = state();
        app.notif.last_slow_sync = check_started;
        app.notif.save_state()?;
        Ok(())
    }
}
