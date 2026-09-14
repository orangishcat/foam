use std::sync::Mutex;

use reqwest::{
    Url,
    cookie::{CookieStore, Jar},
    header::HeaderValue,
};

use crate::config::config_write;

/// Keeps the configured session cookie in sync with the jar, including redirects.
pub(super) struct SessionCookies {
    jar: Mutex<Jar>,
    url: Url,
    key: String,
}

impl SessionCookies {
    pub(super) fn new(url: Url, key: String, value: String) -> Self {
        let jar = Jar::default();
        jar.add_cookie_str(&format!("{key}={value}; Path=/; Secure"), &url);
        Self {
            jar: Mutex::new(jar),
            url,
            key,
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

        let mut config = config_write();
        // Do not overwrite a different account configured since this client was created.
        if config.cookie_key != self.key
            || self.url.host_str()
                != Some(format!("{}.schoology.com", config.subdomain.trim()).as_str())
            || config.cookie_value == value
        {
            return;
        }
        config.cookie_value = value.to_owned();
        // save() logs failures; cookie-store callbacks cannot return an error.
        let _ = config.save();
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        self.jar
            .lock()
            .expect("session cookie jar lock is poisoned")
            .cookies(url)
    }
}
