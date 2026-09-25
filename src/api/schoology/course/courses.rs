use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{super::api_get_with_query, course};
use crate::{
    api::{
        schoology::{
            RequestResult,
            course::{grades::scrape_grades, submissions::scrape_submissions},
        },
        types::{LooseString, LooseUsize},
    },
    config::config,
    database,
    types::{assignment::Assignment, course::Course},
};

const PAGE_LIMIT: usize = 50;

pub fn section_course_title(id: &str) -> RequestResult<String> {
    let section: SchoologyCourse =
        super::super::api_get(&format!("https://api.schoology.com/v1/sections/{id}"))?;
    Ok(section.course_title)
}

#[derive(Serialize, oauth::Request)]
struct CoursesQuery {
    start: usize,
    limit: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct CoursesResponse {
    section: Vec<SchoologyCourse>,
    total: LooseUsize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct SchoologyCourse {
    #[serde(rename = "id")]
    nid: LooseString,
    course_title: String,
    course_code: String,
    #[serde(alias = "title")]
    section_title: String,
    section_code: String,
    active: i64,
    description: String,
    #[serde(rename = "profile_url")]
    logo_img_src: String,
    location: String,
    meeting_days: Vec<Value>,
    start_time: String,
    end_time: String,
    weight: LooseString,
    links: Links,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Links {
    #[serde(rename = "self")]
    self_url: String,
}

/// Fetch every configured-user section and coerce it, including its complete
/// material tree, into unified course models.
pub fn scrape_courses() -> RequestResult<Vec<Course>> {
    let courses = scrape_materials()?;
    scrape_grades()?;
    for course in &courses {
        let mut assignments = database::from_sql::<Assignment>(
            "SELECT * FROM assignments WHERE course_id = ?".to_owned(),
            &[&course.course_id],
        )?;
        scrape_submissions(&mut assignments)?;
        crate::types::submission::update_submissions(&assignments)?;
    }
    Ok(courses)
}

/// Fetch sections and their material trees without requesting grades.
pub fn scrape_materials() -> RequestResult<Vec<Course>> {
    let mut sections = Vec::new();
    let mut start = 0;
    loop {
        let query = CoursesQuery {
            start,
            limit: PAGE_LIMIT,
        };
        let mut page: CoursesResponse = api_get_with_query(
            &format!(
                "https://api.schoology.com/v1/users/{}/sections",
                config().user_id
            ),
            &query,
        )?;
        let page_len = page.section.len();
        let total = page.total.0;
        sections.append(&mut page.section);
        if page_len < PAGE_LIMIT || (total > 0 && sections.len() >= total) {
            break;
        }
        start += page_len;
    }

    sections
        .into_iter()
        .map(|section| {
            let materials = course(&section.nid.0, "0")?;
            let course = Course {
                course_id: section.nid.0,
                aliases: Vec::new(),
                course_title: section.course_title,
                course_code: section.course_code,
                course_url: section.links.self_url,
                section_title: section.section_title,
                section_code: section.section_code,
                active: section.active != 0,
                description: section.description,
                logo_img_src: section.logo_img_src,
                location: section.location,
                period: None,
                hidden: false,
                order: 0,
                meeting_days: section
                    .meeting_days
                    .into_iter()
                    .filter_map(|day| match day {
                        Value::Number(n) => n.as_i64().and_then(|n| i8::try_from(n).ok()),
                        Value::String(s) => s.parse().ok(),
                        _ => None,
                    })
                    .collect(),
                start_time: section.start_time,
                end_time: section.end_time,
                weight: section.weight.0,
            };
            crate::types::course::store_course(&course, &materials)?;
            Ok(course)
        })
        .collect()
}
