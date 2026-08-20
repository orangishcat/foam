use std::{
    sync::{LazyLock, RwLock},
    time::Duration,
};

use reqwest::{
    blocking::Client,
    header::{ACCEPT, AUTHORIZATION, COOKIE},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::config::CONFIG;

pub mod courses;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
static INTERNAL_CLIENT: LazyLock<RwLock<Client>> = LazyLock::new(|| RwLock::new(new_client()));
static API_CLIENT: LazyLock<RwLock<Client>> = LazyLock::new(|| RwLock::new(new_client()));

fn new_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build Schoology HTTP client")
}

fn internal_url(route: &str) -> String {
    let config = CONFIG.read().expect("config lock is poisoned");
    assert!(
        !config.subdomain.trim().is_empty(),
        "Schoology subdomain is not configured"
    );
    format!("https://{}.schoology.com{route}", config.subdomain.trim())
}

fn authorization(method: reqwest::Method, url: &str) -> String {
    let config = CONFIG.read().expect("config lock is poisoned");
    let key = config
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .expect("Schoology API key is not configured");
    let secret = config
        .api_secret
        .as_deref()
        .filter(|secret| !secret.is_empty())
        .expect("Schoology API secret is not configured");
    let token = oauth::Token::from_parts(key, secret, "", "");
    let header = match method {
        reqwest::Method::GET => oauth::get(url, &(), &token, oauth::PLAINTEXT),
        reqwest::Method::POST => oauth::post(url, &(), &token, oauth::PLAINTEXT),
        _ => unreachable!(),
    };
    header.replacen("OAuth ", "OAuth realm=\"Schoology API\",", 1)
}

pub fn internal_get<T: DeserializeOwned>(route: &str) -> T {
    let url = internal_url(route);
    let config = CONFIG.read().expect("config lock is poisoned");
    let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
    INTERNAL_CLIENT
        .read()
        .expect("internal client lock is poisoned")
        .get(url)
        .header(ACCEPT, "application/json")
        .header(COOKIE, cookie)
        .send()
        .expect("Schoology internal GET failed")
        .error_for_status()
        .expect("Schoology internal GET returned an error")
        .json()
        .expect("failed to decode Schoology internal response")
}

pub fn internal_post<B: Serialize + ?Sized, T: DeserializeOwned>(route: &str, body: &B) -> T {
    let url = internal_url(route);
    let config = CONFIG.read().expect("config lock is poisoned");
    let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
    INTERNAL_CLIENT
        .read()
        .expect("internal client lock is poisoned")
        .post(url)
        .header(ACCEPT, "application/json")
        .header(COOKIE, cookie)
        .json(body)
        .send()
        .expect("Schoology internal POST failed")
        .error_for_status()
        .expect("Schoology internal POST returned an error")
        .json()
        .expect("failed to decode Schoology internal response")
}

pub fn api_get<T: DeserializeOwned>(url: &str) -> T {
    let authorization = authorization(reqwest::Method::GET, url);
    API_CLIENT
        .read()
        .expect("API client lock is poisoned")
        .get(url)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, authorization)
        .send()
        .expect("Schoology API GET failed")
        .error_for_status()
        .expect("Schoology API GET returned an error")
        .json()
        .expect("failed to decode Schoology API response")
}

pub fn api_post<B: Serialize + ?Sized, T: DeserializeOwned>(url: &str, body: &B) -> T {
    let authorization = authorization(reqwest::Method::POST, url);
    API_CLIENT
        .read()
        .expect("API client lock is poisoned")
        .post(url)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, authorization)
        .json(body)
        .send()
        .expect("Schoology API POST failed")
        .error_for_status()
        .expect("Schoology API POST returned an error")
        .json()
        .expect("failed to decode Schoology API response")
}
