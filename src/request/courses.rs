use std::fs;

use chrono::Utc;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Deserializer, Serialize};

use super::{RequestError, api_client, internal_client, oauth_authorization};
use crate::config::AppConfig;

const INTERNAL_ROUTE: &str = "/iapi2/site-navigation/courses";
const API_ROUTE: &str = "https://api.schoology.com/v1/users/me/sections";
const DEFAULT_LOGO: &str =
    "https://asset-cdn.schoology.com/sites/all/themes/schoology_theme/images/course-default.svg";

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

pub fn courses(config: &mut AppConfig) -> Result<CoursesResponse, RequestError> {
    let internal_result = if config.subdomain.trim().is_empty() {
        Err("subdomain is empty".to_string())
    } else {
        match internal_client(config) {
            Ok(client) => client
                .get(format!(
                    "https://{}.schoology.com{INTERNAL_ROUTE}",
                    config.subdomain.trim()
                ))
                .send()
                .and_then(|response| response.error_for_status())
                .and_then(|response| response.json::<CoursesResponse>())
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        }
    };

    let response = match internal_result {
        Ok(response) => response,
        Err(internal_error) => {
            let authorization = oauth_authorization(config)?;
            let fallback_result = api_client()?
                .get(API_ROUTE)
                .header(AUTHORIZATION, authorization)
                .header(ACCEPT, "application/json")
                .send()
                .and_then(|response| response.error_for_status())
                .and_then(|response| response.json::<ApiSectionsResponse>());

            let api_response =
                fallback_result.map_err(|fallback_error| RequestError::BothRoutesFailed {
                    internal: internal_error,
                    fallback: fallback_error.to_string(),
                })?;

            CoursesResponse {
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
            }
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
