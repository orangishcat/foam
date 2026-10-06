use std::sync::Mutex;

use reqwest::{
    Url,
    cookie::{CookieStore, Jar},
    header::HeaderValue,
};

use crate::{account::Account, api::schoology::account::SchoologyAccountConfig};

/// Keeps the configured session cookie in sync with the jar, including redirects.
#[derive(Debug)]
pub(super) struct SessionCookies {
    jar: Mutex<Jar>,
    url: Url,
    key: String,
    acc: Mutex<SchoologyAccountConfig>,
}

impl SessionCookies {
    pub(super) fn new(url: Url, account: &SchoologyAccountConfig) -> Self {
        let key = account.cookie_key.clone();
        let value = &account.cookie_value;
        let mut acc = account.clone();
        acc.api_client = Default::default();
        acc.internal_client = Default::default();
        acc.cookies = Default::default();
        let jar = Jar::default();
        jar.add_cookie_str(&format!("{key}={value}; Path=/; Secure"), &url);
        Self {
            jar: Mutex::new(jar),
            url,
            key,
            acc: Mutex::new(acc),
        }
    }
}

impl CookieStore for SessionCookies {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        // Serialize jar updates and persistence so concurrent responses cannot save stale values.
        let jar = self
            .jar
            .lock()
            .expect("session cookie jar lock is poisoned");
        jar.set_cookies(headers, url);
        let cookies = jar.cookies(&self.url);
        let value = cookies
            .as_ref()
            .and_then(|header| header.to_str().ok())
            .and_then(|header| {
                header.split(';').find_map(|pair| {
                    let (name, value) = pair.trim().split_once('=')?;
                    (name == self.key).then_some(value)
                })
            })
            .unwrap_or_default();

        let mut config = self.acc.lock().expect("account cookie lock is poisoned");
        // Persist only the cookie belonging to this account's host and key.
        if config.cookie_key != self.key
            || self.url.host_str()
                != Some(format!("{}.schoology.com", config.subdomain.trim()).as_str())
            || config.cookie_value == value
        {
            return;
        }
        config.cookie_value = value.to_owned();
        // Cookie-store callbacks cannot return an error.
        if let Err(error) = config.save() {
            log::warn!("Failed to save Schoology session cookie: {error}");
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        self.jar
            .lock()
            .expect("session cookie jar lock is poisoned")
            .cookies(url)
    }
}
