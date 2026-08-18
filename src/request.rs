use std::{
    error::Error,
    fmt, fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{
    blocking::Client,
    header::{ACCEPT, AUTHORIZATION, COOKIE, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Deserializer, Serialize};

use crate::config::AppConfig;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const DEFAULT_LOGO: &str =
    "https://asset-cdn.schoology.com/sites/all/themes/schoology_theme/images/course-default.svg";
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoursesResponse {
    pub data: CoursesData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoursesData {
    pub courses: Vec<Course>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub nid: u64,
    pub course_title: String,
    pub section_title: String,
    pub building_title: String,
    pub logo_img_src: String,
    pub course_nid: u64,
    pub weight: i64,
    pub course_landing_page_type: String,
    pub is_csl: bool,
    pub admin_type: String,
}

#[derive(Debug)]
pub enum CoursesError {
    InvalidConfiguration(&'static str),
    InvalidHeader(reqwest::header::InvalidHeaderValue),
    Request(reqwest::Error),
    Storage(std::io::Error),
    Serialization(serde_json::Error),
    BothRoutesFailed { internal: String, fallback: String },
}

impl fmt::Display for CoursesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => write!(formatter, "{message}"),
            Self::InvalidHeader(error) => write!(formatter, "invalid request header: {error}"),
            Self::Request(error) => write!(formatter, "course request failed: {error}"),
            Self::Storage(error) => write!(formatter, "failed to store course data: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "failed to serialize course data: {error}")
            }
            Self::BothRoutesFailed { internal, fallback } => write!(
                formatter,
                "both Schoology course routes failed (internal: {internal}; API fallback: {fallback})"
            ),
        }
    }
}

impl Error for CoursesError {}

impl From<reqwest::Error> for CoursesError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(error)
    }
}

impl From<std::io::Error> for CoursesError {
    fn from(error: std::io::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<serde_json::Error> for CoursesError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

#[derive(Deserialize)]
struct ApiSectionsResponse {
    #[serde(default)]
    section: Vec<ApiSection>,
}

#[derive(Deserialize)]
struct ApiSection {
    #[serde(deserialize_with = "deserialize_u64")]
    id: u64,
    #[serde(deserialize_with = "deserialize_u64")]
    course_id: u64,
    #[serde(default)]
    course_title: String,
    #[serde(default, alias = "title")]
    section_title: String,
    #[serde(default)]
    profile_url: String,
    #[serde(default, deserialize_with = "deserialize_i64_default")]
    weight: i64,
    #[serde(default, deserialize_with = "deserialize_i64_default")]
    admin: i64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IntegerValue {
    Integer(u64),
    String(String),
}

fn deserialize_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match IntegerValue::deserialize(deserializer)? {
        IntegerValue::Integer(value) => Ok(value),
        IntegerValue::String(value) => value.parse().map_err(serde::de::Error::custom),
    }
}

fn deserialize_i64_default<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Integer(i64),
        String(String),
        Null,
    }

    match Value::deserialize(deserializer)? {
        Value::Integer(value) => Ok(value),
        Value::String(value) => value.parse().map_err(serde::de::Error::custom),
        Value::Null => Ok(0),
    }
}

pub fn courses(config: &mut AppConfig) -> Result<CoursesResponse, CoursesError> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    if !config.cookie_key.is_empty() {
        let cookie = format!("{}={}", config.cookie_key, config.cookie_value);
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie).map_err(CoursesError::InvalidHeader)?,
        );
    }

    let internal_result = if config.subdomain.trim().is_empty() {
        Err("subdomain is empty".to_string())
    } else {
        let internal_client = Client::builder()
            .default_headers(headers)
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| error.to_string());
        match internal_client {
            Ok(client) => client
                .get(format!(
                    "https://{}.schoology.com/iapi2/site-navigation/courses",
                    config.subdomain.trim()
                ))
                .send()
                .and_then(|response| response.error_for_status())
                .and_then(|response| response.json::<CoursesResponse>())
                .map_err(|error| error.to_string()),
            Err(error) => Err(error),
        }
    };

    let response = match internal_result {
        Ok(response) => response,
        Err(internal_error) => {
            let api_key = config
                .api_key
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or(CoursesError::InvalidConfiguration(
                    "the internal course request failed and api_key is not configured",
                ))?;
            let api_secret = config
                .api_secret
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or(CoursesError::InvalidConfiguration(
                    "the internal course request failed and api_secret is not configured",
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
            let authorization = format!(
                "OAuth realm=\"Schoology API\", oauth_consumer_key=\"{key}\", oauth_token=\"\", oauth_nonce=\"{nonce}\", oauth_timestamp=\"{timestamp}\", oauth_signature_method=\"PLAINTEXT\", oauth_version=\"1.0\", oauth_signature=\"{secret}%26\""
            );

            let fallback_result = (|| -> Result<CoursesResponse, reqwest::Error> {
                let client = Client::builder()
                    .user_agent(USER_AGENT)
                    .timeout(Duration::from_secs(30))
                    .build()?;
                let api_response = client
                    .get("https://api.schoology.com/v1/users/me/sections")
                    .header(AUTHORIZATION, authorization)
                    .header(ACCEPT, "application/json")
                    .send()?
                    .error_for_status()?
                    .json::<ApiSectionsResponse>()?;

                Ok(CoursesResponse {
                    data: CoursesData {
                        courses: api_response
                            .section
                            .into_iter()
                            .map(|section| Course {
                                nid: section.id,
                                course_title: section.course_title,
                                section_title: section.section_title,
                                building_title: String::new(),
                                logo_img_src: if section.profile_url.is_empty() {
                                    DEFAULT_LOGO.to_string()
                                } else {
                                    section.profile_url
                                },
                                course_nid: section.course_id,
                                weight: section.weight,
                                course_landing_page_type: "materials".to_string(),
                                is_csl: false,
                                admin_type: if section.admin == 0 {
                                    "none".to_string()
                                } else {
                                    "admin".to_string()
                                },
                            })
                            .collect(),
                    },
                })
            })();

            fallback_result.map_err(|fallback_error| CoursesError::BothRoutesFailed {
                internal: internal_error,
                fallback: fallback_error.to_string(),
            })?
        }
    };

    for course in &response.data.courses {
        let course_dir = config
            .data_dir()
            .join("courses")
            .join(course.nid.to_string());
        fs::create_dir_all(&course_dir)?;
        let contents = serde_json::to_string_pretty(course)?;
        fs::write(course_dir.join("course.json"), format!("{contents}\n"))?;
    }

    config.last_updated = Some(Utc::now());
    config.save()?;
    Ok(response)
}
