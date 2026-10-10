use std::{io, sync::Arc, time::Duration};

use reqwest::{
    blocking::Client,
    cookie::CookieStore,
    header::{ACCEPT, AUTHORIZATION, COOKIE, USER_AGENT as USER_AGENT_HEADER},
};
use serde::{Serialize, de::DeserializeOwned};
use wry::http::HeaderMap;

use crate::{
    account::{Account, RequestResult},
    api::schoology::{account::SchoologyAccountConfig, cookies},
};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

impl SchoologyAccountConfig {
    pub fn resource_headers(&self, url: &str) -> RequestResult<HeaderMap> {
        let request = self.get_resource(url)?.build()?;
        let mut headers = request.headers().clone();
        headers.insert(USER_AGENT_HEADER, USER_AGENT.parse()?);
        if !url.starts_with("https://api.schoology.com") {
            let jar = self.session_cookies()?;
            if let Some(cookie) = jar.cookies(request.url()) {
                headers.insert(COOKIE, cookie);
            }
        }
        Ok(headers)
    }

    pub fn internal_get_request(
        &self,
        url: &str,
    ) -> RequestResult<reqwest::blocking::RequestBuilder> {
        let request = self.internal_client()?.get(url);
        Ok(request.header(ACCEPT, "application/json"))
    }

    pub fn api_get_request(&self, url: &str) -> RequestResult<reqwest::blocking::RequestBuilder> {
        let authorization = self.authorization(reqwest::Method::GET, url, &())?;
        let request = self.api_client()?.get(url);
        Ok(request
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, authorization))
    }

    pub(crate) fn new_client() -> RequestResult<Client> {
        Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(Into::into)
    }

    pub(crate) fn new_internal_client(&self) -> RequestResult<Client> {
        let jar = self.session_cookies()?;

        Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .cookie_provider(jar)
            .build()
            .map_err(Into::into)
    }

    fn internal_url(&self, route: &str) -> RequestResult<String> {
        if self.subdomain.trim().is_empty() {
            return Err(io::Error::other("Schoology subdomain is not configured").into());
        }
        Ok(format!(
            "https://{}.schoology.com{route}",
            self.subdomain.trim()
        ))
    }

    fn authorization<R: oauth::Request + ?Sized>(
        &self,
        method: reqwest::Method,
        url: &str,
        request: &R,
    ) -> RequestResult<String> {
        let key = self
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| io::Error::other("Schoology API key is not configured"))?;
        let secret = self
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

    pub fn internal_get<T: DeserializeOwned>(&self, route: &str) -> RequestResult<T> {
        let url = self.internal_url(route)?;
        self.internal_get_request(&url)?
            .send()?
            .error_for_status()?
            .json()
            .map_err(Into::into)
    }

    pub fn api_get<T: DeserializeOwned>(&self, url: &str) -> RequestResult<T> {
        self.api_get_request(url)?
            .send()?
            .error_for_status()?
            .json()
            .map_err(Into::into)
    }

    pub fn api_get_with_query<Q, T>(&self, url: &str, query: &Q) -> RequestResult<T>
    where
        Q: oauth::Request + Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let authorization = self.authorization(reqwest::Method::GET, url, query)?;
        let text = self
            .api_client()?
            .get(url)
            .query(query)
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, authorization)
            .send()?
            .error_for_status()?
            .text()?;
        log::debug!("response for {url}: {}", text);
        serde_json::from_str(&text).map_err(Into::into)
    }

    pub fn api_post<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> RequestResult<T> {
        let authorization = self.authorization(reqwest::Method::POST, url, &())?;
        self.api_client()?
            .post(url)
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, authorization)
            .json(body)
            .send()?
            .error_for_status()?
            .json()
            .map_err(Into::into)
    }

    fn api_client(&self) -> RequestResult<&Client> {
        self.api_client
            .get_or_init(Self::new_client)
            .as_ref()
            .map_err(|error| io::Error::other(error.to_string()).into())
    }

    fn internal_client(&self) -> RequestResult<&Client> {
        self.internal_client
            .get_or_init(|| self.new_internal_client())
            .as_ref()
            .map_err(|error| io::Error::other(error.to_string()).into())
    }

    fn session_cookies(&self) -> RequestResult<Arc<cookies::SessionCookies>> {
        self.cookies
            .get_or_init(|| {
                let url = self.internal_url("/")?.parse()?;
                Ok(Arc::new(cookies::SessionCookies::new(url, self)))
            })
            .as_ref()
            .cloned()
            .map_err(|error| io::Error::other(error.to_string()).into())
    }
}
