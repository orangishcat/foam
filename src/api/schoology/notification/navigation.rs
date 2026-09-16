use std::collections::HashSet;

use chrono::{DateTime, Days, Local, NaiveTime, TimeZone};

use super::super::{RequestResult, internal_get};
use crate::{
    api::schoology::types::notification::{NotificationsResponse, SchoologyNotification},
    types::{
        material::MaterialType,
        notification::{Notification, NotificationEvent},
    },
};

const ROUTE: &str = "/iapi2/site-navigation/notifications";

pub fn scrape_notifications() -> RequestResult<Vec<Notification>> {
    let response: NotificationsResponse = internal_get(ROUTE)?;
    let now = Local::now();
    Ok(response
        .data
        .into_iter()
        .flat_map(|item| item.into_notifications(now))
        .collect())
}

impl SchoologyNotification {
    fn into_notifications(self, now: DateTime<Local>) -> Vec<Notification> {
        let event = match self.kind.as_str() {
            "course_materials_add" => NotificationEvent::MaterialPosted,
            "grade_add" | "grade_update" => NotificationEvent::GradeUpdated,
            _ => return Vec::new(),
        };
        let section = self
            .args
            .iter()
            .find(|arg| arg.kind == "s_content_course_section");
        let course_id = section.map(|arg| arg.id.0.clone()).unwrap_or_default();
        let course_title = section
            .map(|arg| decode_title(&arg.title))
            .unwrap_or_default();
        let created = parse_created(&self.created, now);
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for arg in self.args.iter().filter(|arg| {
            arg.kind == "s_content_grade_item"
                || (event == NotificationEvent::MaterialPosted
                    && arg.kind == "s_content_generic_post")
        }) {
            if !seen.insert((&arg.kind, &arg.id.0)) {
                continue;
            }
            result.push(Notification {
                event,
                title: decode_title(&arg.title),
                viewed: self.viewed,
                created,
                resource_id: arg.id.0.clone(),
                material_type: match (arg.kind.as_str(), arg.document_type.as_str()) {
                    ("s_content_grade_item", _) => Some(MaterialType::Assignment),
                    (_, "file") => Some(MaterialType::Document),
                    (_, "link") => Some(MaterialType::Link),
                    _ => None,
                },
                course_id: course_id.clone(),
                course_title: course_title.clone(),
                is_processed: false,
            });
        }
        result
    }
}

fn decode_title(value: &str) -> String {
    scraper::Html::parse_fragment(value)
        .root_element()
        .text()
        .collect()
}

fn parse_created(value: &str, now: DateTime<Local>) -> DateTime<Local> {
    if let Some(relative) = value.strip_suffix(" ago") {
        let mut parts = relative.split_whitespace();
        if let (Some(amount), Some(unit)) = (
            parts.next().and_then(|v| v.parse::<i64>().ok()),
            parts.next(),
        ) {
            let duration = match unit {
                "second" | "seconds" => chrono::Duration::try_seconds(amount),
                "minute" | "minutes" => chrono::Duration::try_minutes(amount),
                "hour" | "hours" => chrono::Duration::try_hours(amount),
                "day" | "days" => chrono::Duration::try_days(amount),
                _ => None,
            };
            if let Some(parsed) = duration.and_then(|d| now.checked_sub_signed(d)) {
                return parsed;
            }
        }
    }
    let parsed = (|| {
        let (day, time) = value.split_once(" at ")?;
        let date = match day {
            "Today" => now.date_naive(),
            "Yesterday" => now.date_naive().checked_sub_days(Days::new(1))?,
            _ => return None,
        };
        let time = NaiveTime::parse_from_str(time, "%I:%M %P").ok()?;
        Local.from_local_datetime(&date.and_time(time)).single()
    })();
    parsed.unwrap_or(now)
}
