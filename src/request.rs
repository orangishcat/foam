use std::sync::Mutex;
use std::time::Duration;

use reqwest::blocking::{Client, Response};

use crate::config::AppConfig;

static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

// pretend we're chrome or something, idk
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn init(config: &AppConfig) -> reqwest::Result<()> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()?;
    let mut guard = CLIENT.lock().unwrap();
    *guard = Some(client);
    Ok(())
}

fn client() -> Client {
    CLIENT
        .lock()
        .unwrap()
        .clone()
        .expect("client not initialized")
}

pub fn get(url: &str) -> reqwest::Result<Response> {
    client().get(url).send()
}

pub fn post(url: &str) -> reqwest::Result<Response> {
    client().post(url).send()
}
