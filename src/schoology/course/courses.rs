use std::{collections::BTreeMap, fs, path::PathBuf};

use chrono::Utc;
use log::{debug, error, info, warn};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    config::{config, config_write},
    types::loose_str::LooseString,
};

use super::super::api_get_with_query;

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
    /// Resolved local directory; not part of the Schoology API response.
    #[serde(skip)]
    pub data_dir: PathBuf,
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
    #[serde(default)]
    pub course_id: LooseString,
    #[serde(default)]
    pub school_id: LooseString,
    #[serde(default)]
    pub access_code: String,
    #[serde(default, alias = "title")]
    pub section_title: String,
    #[serde(default)]
    pub section_code: String,
    #[serde(default)]
    pub section_school_code: String,
    #[serde(default)]
    pub synced: LooseString,
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
    #[serde(default)]
    pub weight: LooseString,
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

/// Scrapes every course section belonging to the configured user.
///
/// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/course-section/>
pub fn courses() -> CoursesResponse {
    info!("scraping Schoology course sections");
    let mut response = CoursesResponse::default();
    let mut start = 0;

    loop {
        debug!("scraping Schoology course sections page: start={start}, limit={PAGE_LIMIT}");
        let query = CoursesQuery {
            start,
            limit: PAGE_LIMIT,
        };
        let page_result = api_get_with_query(
            &format!(
                "https://api.schoology.com/v1/users/{}/sections",
                config().user_id
            ),
            &query,
        );
        let mut page: CoursesResponse = match page_result {
            Ok(page) => page,
            Err(error) => {
                warn!("stopping course pagination after failed page at start={start}: {error}");
                break;
            }
        };
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
    let courses_dir = config.data_dir().join("courses");
    if let Err(error) = fs::create_dir_all(&courses_dir) {
        error!("skipping course saves because the courses directory failed: {error}");
        response.section.clear();
        return response;
    }
    let mut saved_courses = Vec::with_capacity(response.section.len());
    for mut course in response.section.drain(..) {
        let title = format!("{}__{}", course.course_title, course.section_title);
        course.data_dir = match super::deduplicated_folder(&courses_dir, &title) {
            Ok(path) => path,
            Err(error) => {
                warn!(
                    "skipping failed course directory for {}: {error}",
                    course.nid
                );
                continue;
            }
        };
        info!(
            "saving Schoology course section: id={}, title={}",
            course.nid, course.section_title
        );
        match serde_json::to_string_pretty(&course) {
            Ok(contents) => {
                if let Err(error) =
                    fs::write(course.data_dir.join("course.json"), format!("{contents}\n"))
                {
                    warn!("skipping failed course save for {}: {error}", course.nid);
                    continue;
                }
            }
            Err(error) => {
                warn!(
                    "skipping failed course serialization for {}: {error}",
                    course.nid
                );
                continue;
            }
        }
        saved_courses.push(course);
    }
    response.section = saved_courses;

    config.last_updated = Some(Utc::now());
    if let Err(error) = config.save() {
        error!("failed to save scraper configuration: {error}");
    }
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
