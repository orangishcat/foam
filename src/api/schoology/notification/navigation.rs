use std::collections::HashSet;

use chrono::{DateTime, Days, Local, NaiveTime, TimeZone};

use super::super::{RequestResult, internal_get};
use crate::{
    api::schoology::types::notification::{NotificationsResponse, SchoologyNotification},
    types::notification::{Notification, NotificationEvent},
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
        // Resource argument types describe the material, not the operation.
        // Only grade events belong to this feed's contribution.
        if !matches!(self.kind.as_str(), "grade_add" | "grade_update") {
            return Vec::new();
        }
        let course_id = if self.realm == "course" {
            self.realm_id.0.clone()
        } else {
            self.args
                .iter()
                .find(|arg| arg.kind == "s_content_course_section")
                .map(|arg| arg.id.0.clone())
                .unwrap_or_default()
        };
        let created = parse_created(&self.created, now);
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for arg in self.args.iter().filter(|arg| !arg.is_context()) {
            if !seen.insert((&arg.kind, &arg.id.0)) {
                continue;
            }
            result.push(Notification {
                event: NotificationEvent::GradeUpdated,
                title: arg.title.clone(),
                viewed: self.viewed,
                created,
                resource_id: arg.id.0.clone(),
                material_type: super::material_type::parse(&arg.kind),
                course_id: course_id.clone(),
                is_processed: false,
            });
        }
        if result.is_empty() {
            result.push(Notification {
                event: NotificationEvent::GradeUpdated,
                title: String::new(),
                viewed: self.viewed,
                created,
                resource_id: String::new(),
                material_type: None,
                course_id,
                is_processed: false,
            });
        }
        result
    }
}

fn parse_created(value: &str, now: DateTime<Local>) -> DateTime<Local> {
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
