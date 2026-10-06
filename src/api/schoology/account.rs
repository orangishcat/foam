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

    fn scrape_courses(&self) -> RequestResult<Vec<crate::types::course::Course>> {
        Self::scrape_courses(self)
    }

    fn scrape_notifications(&self) -> RequestResult<Vec<crate::types::notification::Notification>> {
        Self::scrape_notifications(self)
    }

    fn update_notifications(
        &self,
        notifications: &mut [crate::types::notification::Notification],
        progress: &mut dyn FnMut(f32),
    ) -> RequestResult<()> {
        Self::update_notifications(self, notifications, progress)
    }

    fn fetch_calendar(
        &self,
    ) -> RequestResult<(
        Vec<String>,
        Vec<crate::api::schoology::calendar::CalendarAssignment>,
    )> {
        Self::fetch_calendar(self)
    }

    fn apply_calendar(
        &self,
        ids: &[String],
        assignments: &[crate::api::schoology::calendar::CalendarAssignment],
        progress: &mut dyn FnMut(f32),
    ) -> io::Result<usize> {
        Self::apply_calendar(self, ids, assignments, progress)
    }

    fn scrape_submissions(
        &self,
        assignments: &mut [crate::types::assignment::Assignment],
    ) -> RequestResult<()> {
        Self::scrape_submissions(self, assignments)
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

    fn fast_sync(&self) -> Result<(), std::io::Error> {
        self.scrape_notifications().map_err(io::Error::other)?;
        let (ids, assignments) = self.fetch_calendar().map_err(io::Error::other)?;
        self.apply_calendar(&ids, &assignments, |_| {})?;
        Ok(())
    }

    fn slow_sync(&self) -> Result<(), std::io::Error> {
        let mut uncompleted_assignments = database::from_sql::<crate::types::assignment::Assignment>(
            format!(
                "SELECT * FROM assignments WHERE NOT ({})",
                include_str!("../../sql/assignment/completed.sql")
            ),
            &[],
        )?;
        self.scrape_submissions(&mut uncompleted_assignments)
            .map_err(io::Error::other)?;
        crate::types::submission::update_submissions(&uncompleted_assignments)?;
        Ok(())
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
