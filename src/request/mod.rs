use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{
    blocking::Client,
    header::{ACCEPT, COOKIE, HeaderMap, HeaderValue},
};

use crate::config::AppConfig;

pub mod courses;

pub use courses::courses;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum RequestError {
    InvalidConfiguration(&'static str),
    InvalidHeader(reqwest::header::InvalidHeaderValue),
    Request(reqwest::Error),
    Storage(std::io::Error),
    Serialization(serde_json::Error),
    BothRoutesFailed { internal: String, fallback: String },
}

impl fmt::Display for RequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => write!(formatter, "{message}"),
            Self::InvalidHeader(error) => write!(formatter, "invalid request header: {error}"),
            Self::Request(error) => write!(formatter, "request failed: {error}"),
            Self::Storage(error) => write!(formatter, "failed to store response data: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "failed to serialize response data: {error}")
            }
            Self::BothRoutesFailed { internal, fallback } => write!(
                formatter,
                "both Schoology routes failed (internal: {internal}; API fallback: {fallback})"
            ),
        }
    }
}

impl Error for RequestError {}

impl From<reqwest::Error> for RequestError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(error)
    }
}

impl From<std::io::Error> for RequestError {
    fn from(error: std::io::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<serde_json::Error> for RequestError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

fn internal_client(config: &AppConfig) -> Result<Client, RequestError> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    if !config.cookie_key.is_empty() {
        let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie).map_err(RequestError::InvalidHeader)?,
        );
    }

    Client::builder()
        .default_headers(headers)
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(RequestError::Request)
}

fn api_client() -> Result<Client, RequestError> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(RequestError::Request)
}

fn oauth_authorization(config: &AppConfig) -> Result<String, RequestError> {
    let api_key = config
        .api_key
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or(RequestError::InvalidConfiguration(
            "the internal request failed and api_key is not configured",
        ))?;
    let api_secret = config
        .api_secret
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or(RequestError::InvalidConfiguration(
            "the internal request failed and api_secret is not configured",
        ))?;

    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch");
    let timestamp = elapsed.as_secs();
    let nonce = format!(
        "{}-{}-{}",
        timestamp,
        elapsed.subsec_nanos(),
        NONCE_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let key = utf8_percent_encode(api_key, NON_ALPHANUMERIC);
    let secret = utf8_percent_encode(api_secret, NON_ALPHANUMERIC);

    Ok(format!(
        "OAuth realm=\"Schoology API\", oauth_consumer_key=\"{key}\", oauth_token=\"\", oauth_nonce=\"{nonce}\", oauth_timestamp=\"{timestamp}\", oauth_signature_method=\"PLAINTEXT\", oauth_version=\"1.0\", oauth_signature=\"{secret}%26\""
    ))
}
