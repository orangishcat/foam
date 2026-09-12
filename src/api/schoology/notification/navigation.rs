use std::collections::HashSet;

use chrono::{DateTime, Days, Local, NaiveTime, TimeZone};

use super::super::{RequestResult, internal_get};
use crate::{
    api::schoology::types::notification::{NotificationsResponse, SchoologyNotification},
    types::notification::Notification,
};

const ROUTE: &str = "/iapi2/site-navigation/notifications";

pub(super) fn scrape_notifications() -> RequestResult<Vec<Notification>> {
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
        let placeholders: Vec<_> = self.sentence.split("%s").collect();
        let context_count = self.args.iter().take_while(|arg| arg.is_context()).count();
        let mut prefix = placeholders.first().copied().unwrap_or_default().to_owned();
        for index in 0..context_count {
            prefix.push_str(&self.args[index].title);
            prefix.push_str(placeholders.get(index + 1).copied().unwrap_or_default());
        }
        let suffix = placeholders
            .last()
            .copied()
            .filter(|_| placeholders.len() > context_count + 1)
            .unwrap_or_default();
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for arg in self.args.iter().filter(|arg| !arg.is_context()) {
            if !seen.insert((&arg.kind, &arg.id.0)) {
                continue;
            }
            result.push(Notification {
                title: format!("{prefix}{}{suffix}", arg.title),
                viewed: self.viewed,
                created,
                resource_id: arg.id.0.clone(),
                course_id: course_id.clone(),
            });
        }
        if result.is_empty() {
            result.push(Notification {
                title: prefix,
                viewed: self.viewed,
                created,
                resource_id: String::new(),
                course_id,
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
