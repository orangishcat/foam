use std::{
    error::Error,
    io,
    sync::{LazyLock, RwLock},
    time::Duration,
};

use reqwest::{
    blocking::Client,
    header::{ACCEPT, AUTHORIZATION, COOKIE},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::config::config;

pub mod course;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
pub type RequestError = Box<dyn Error + Send + Sync>;
pub type RequestResult<T> = Result<T, RequestError>;

static INTERNAL_CLIENT: LazyLock<RequestResult<RwLock<Client>>> =
    LazyLock::new(|| new_client().map(RwLock::new));
static API_CLIENT: LazyLock<RequestResult<RwLock<Client>>> =
    LazyLock::new(|| new_client().map(RwLock::new));

fn new_client() -> RequestResult<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(Into::into)
}

fn internal_url(route: &str) -> RequestResult<String> {
    let config = config();
    if config.subdomain.trim().is_empty() {
        return Err(io::Error::other("Schoology subdomain is not configured").into());
    }
    Ok(format!(
        "https://{}.schoology.com{route}",
        config.subdomain.trim()
    ))
}

fn authorization<R: oauth::Request + ?Sized>(
    method: reqwest::Method,
    url: &str,
    request: &R,
) -> RequestResult<String> {
    let config = config();
    let key = config
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| io::Error::other("Schoology API key is not configured"))?;
    let secret = config
        .api_secret
        .as_deref()
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| io::Error::other("Schoology API secret is not configured"))?;
    let token = oauth::Token::from_parts(key, secret, "", "");
    let header = match method {
        reqwest::Method::GET => oauth::get(url, request, &token, oauth::PLAINTEXT),
        reqwest::Method::POST => oauth::post(url, request, &token, oauth::PLAINTEXT),
        _ => return Err(io::Error::other("unsupported OAuth request method").into()),
    };
    Ok(header.replacen("OAuth ", "OAuth realm=\"Schoology API\",", 1))
}

pub fn internal_get<T: DeserializeOwned>(route: &str) -> RequestResult<T> {
    let url = internal_url(route)?;
    let config = config();
    let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
    INTERNAL_CLIENT
        .as_ref()
        .map_err(|error| io::Error::other(error.to_string()))?
        .read()
        .map_err(|_| io::Error::other("internal client lock is poisoned"))?
        .get(url)
        .header(ACCEPT, "application/json")
        .header(COOKIE, cookie)
        .send()?
        .error_for_status()?
        .json()
        .map_err(Into::into)
}

pub fn internal_post<B: Serialize + ?Sized, T: DeserializeOwned>(
    route: &str,
    body: &B,
) -> RequestResult<T> {
    let url = internal_url(route)?;
    let config = config();
    let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
    INTERNAL_CLIENT
        .as_ref()
        .map_err(|error| io::Error::other(error.to_string()))?
        .read()
        .map_err(|_| io::Error::other("internal client lock is poisoned"))?
        .post(url)
        .header(ACCEPT, "application/json")
        .header(COOKIE, cookie)
        .json(body)
        .send()?
        .error_for_status()?
        .json()
        .map_err(Into::into)
}

pub fn api_get<T: DeserializeOwned>(url: &str) -> RequestResult<T> {
    let authorization = authorization(reqwest::Method::GET, url, &())?;
    API_CLIENT
        .as_ref()
        .map_err(|error| io::Error::other(error.to_string()))?
        .read()
        .map_err(|_| io::Error::other("API client lock is poisoned"))?
        .get(url)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, authorization)
        .send()?
        .error_for_status()?
        .json()
        .map_err(Into::into)
}

pub fn api_get_with_query<Q, T>(url: &str, query: &Q) -> RequestResult<T>
where
    Q: oauth::Request + Serialize + ?Sized,
    T: DeserializeOwned,
{
    let authorization = authorization(reqwest::Method::GET, url, query)?;
    API_CLIENT
        .as_ref()
        .map_err(|error| io::Error::other(error.to_string()))?
        .read()
        .map_err(|_| io::Error::other("API client lock is poisoned"))?
        .get(url)
        .query(query)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, authorization)
        .send()?
        .error_for_status()?
        .json()
        .map_err(Into::into)
}

pub fn api_post<B: Serialize + ?Sized, T: DeserializeOwned>(
    url: &str,
    body: &B,
) -> RequestResult<T> {
    let authorization = authorization(reqwest::Method::POST, url, &())?;
    API_CLIENT
        .as_ref()
        .map_err(|error| io::Error::other(error.to_string()))?
        .read()
        .map_err(|_| io::Error::other("API client lock is poisoned"))?
        .post(url)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, authorization)
        .json(body)
        .send()?
        .error_for_status()?
        .json()
        .map_err(Into::into)
}
