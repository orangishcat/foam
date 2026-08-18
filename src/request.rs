use std::time::Duration;

use reqwest::{
    blocking::{Client, Response},
    header::{COOKIE, HeaderMap, HeaderValue},
};

use crate::config::AppConfig;
use std::sync::OnceLock;

static CLIENT: OnceLock<Client> = OnceLock::new();

// pretend we're chrome or something, idk
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn init(config: &AppConfig) -> reqwest::Result<()> {
    let mut header_map = HeaderMap::new();
    let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
    header_map.append(
        COOKIE,
        HeaderValue::from_str(&cookie).expect("invalid cookie value"),
    );

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()?;
    CLIENT.set(client).expect("client already initialized");
    Ok(())
}

fn client() -> &'static Client {
    CLIENT.get().expect("client not initialized")
}

pub fn get(url: &str) -> reqwest::Result<Response> {
    client().get(url).send()
}

pub fn post(url: &str) -> reqwest::Result<Response> {
    client().post(url).send()
}
