use std::{collections::BTreeMap, fs};

use chrono::Utc;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::config::{config, config_write};

use super::api_get_with_query;

const PAGE_LIMIT: usize = 50;

#[derive(Serialize, oauth::Request)]
struct CoursesQuery {
    start: usize,
    limit: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoursesResponse {
    #[serde(default)]
    pub section: Vec<Course>,
    #[serde(default, deserialize_with = "deserialize_usize_default")]
    pub total: usize,
    #[serde(default)]
    pub links: Links,
}

/// A Schoology course section returned by `users/{user_id}/sections`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Course {
    #[serde(
        default,
        rename = "id",
        deserialize_with = "deserialize_string_default"
    )]
    pub nid: String,
    #[serde(default)]
    pub course_title: String,
    #[serde(default)]
    pub course_code: String,
    #[serde(default, deserialize_with = "deserialize_string_default")]
    pub course_id: String,
    #[serde(default, deserialize_with = "deserialize_string_default")]
    pub school_id: String,
    #[serde(default)]
    pub access_code: String,
    #[serde(default, alias = "title")]
    pub section_title: String,
    #[serde(default)]
    pub section_code: String,
    #[serde(default)]
    pub section_school_code: String,
    #[serde(default, deserialize_with = "deserialize_string_default")]
    pub synced: String,
    #[serde(default)]
    pub active: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub grading_periods: Vec<i64>,
    #[serde(default, rename = "profile_url")]
    pub logo_img_src: String,
    #[serde(default)]
    pub location: String,
    // The API docs say integers, but their own example contains [""].
    #[serde(default)]
    pub meeting_days: Vec<Value>,
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
    #[serde(default, deserialize_with = "deserialize_string_default")]
    pub weight: String,
    #[serde(default)]
    pub options: Value,
    #[serde(default)]
    pub links: Links,
    #[serde(default)]
    pub admin: i64,
    /// Preserve fields added by Schoology without preventing deserialization.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Links {
    #[serde(default, rename = "self")]
    pub self_url: String,
    #[serde(default)]
    pub next: Option<String>,
}

pub fn courses() -> CoursesResponse {
    let mut response = CoursesResponse::default();
    let mut start = 0;

    loop {
        let query = CoursesQuery {
            start,
            limit: PAGE_LIMIT,
        };
        let mut page: CoursesResponse = api_get_with_query(
            &format!(
                "https://api.schoology.com/v1/users/{}/sections",
                &config().user_id
            ),
            &query,
        );
        let page_len = page.section.len();

        if response.links.self_url.is_empty() {
            response.links = page.links.clone();
        }
        response.total = response.total.max(page.total);
        response.section.append(&mut page.section);

        if page_len < PAGE_LIMIT || (response.total > 0 && response.section.len() >= response.total)
        {
            break;
        }
        start += page_len;
    }

    if response.total == 0 {
        response.total = response.section.len();
    }

    let mut config = config_write();
    for course in &response.section {
        let course_dir = config.data_dir().join("courses").join(&course.nid);
        fs::create_dir_all(&course_dir).expect("failed to create course data directory");
        let contents = serde_json::to_string_pretty(course).expect("failed to serialize course");
        fs::write(course_dir.join("course.json"), format!("{contents}\n"))
            .expect("failed to save course");
    }

    config.last_updated = Some(Utc::now());
    config.save().expect("failed to save configuration");
    response
}

fn deserialize_string_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(value) => Ok(value),
        Value::Number(value) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        value => Err(serde::de::Error::custom(format!(
            "expected a string or number, got {value}"
        ))),
    }
}

fn deserialize_usize_default<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Number(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| serde::de::Error::custom("total is outside usize range")),
        Value::String(value) => value.parse().map_err(serde::de::Error::custom),
        Value::Null => Ok(0),
        value => Err(serde::de::Error::custom(format!(
            "expected a string or number, got {value}"
        ))),
    }
}
