use chrono::{DateTime, Days, Local, NaiveTime, TimeZone};
use serde::Deserialize;

use super::super::{RequestResult, internal_get};
use crate::types::{LooseString, notification::Notification};

#[derive(Deserialize)]
struct NotificationsResponse {
    data: Vec<SchoologyNotification>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchoologyNotification {
    sentence: String,
    viewed: bool,
    created: String,
    more: String,
    realm: String,
    realm_id: LooseString,
    args: Vec<NotificationArgument>,
}

#[derive(Deserialize)]
struct NotificationArgument {
    id: LooseString,
    title: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Fetch the navigation notification feed using the configured session cookie.
pub fn scrape_notifications() -> RequestResult<Vec<Notification>> {
    let response: NotificationsResponse = internal_get("/iapi2/site-navigation/notifications")?;
    let fetched_at = Local::now();
    Ok(response
        .data
        .into_iter()
        .map(|item| item.into_notification(fetched_at))
        .collect())
}

impl SchoologyNotification {
    fn into_notification(self, fetched_at: DateTime<Local>) -> Notification {
        // Replace original placeholders only: titles may themselves contain `%s`.
        let mut parts = self.sentence.split("%s");
        let mut title = parts.next().unwrap_or_default().to_owned();
        for (index, part) in parts.enumerate() {
            title.push_str(self.args.get(index).map_or("%s", |arg| arg.title.as_str()));
            title.push_str(part);
        }
        if !self.more.is_empty() {
            title.push_str(" (");
            title.push_str(&self.more);
            title.push(')');
        }
        let course = self
            .args
            .iter()
            .find(|arg| arg.kind == "s_content_course_section");
        let course_id = if self.realm == "course" {
            self.realm_id.0
        } else {
            course.map(|arg| arg.id.0.clone()).unwrap_or_default()
        };
        let resource_id = self
            .args
            .iter()
            .find(|arg| arg.kind != "s_content_course_section")
            .map(|arg| arg.id.0.clone())
            .unwrap_or_default();
        Notification {
            title,
            viewed: self.viewed,
            created: parse_created(&self.created, fetched_at),
            resource_id,
            course_id,
        }
    }
}

// The feed supplies display text, not an absolute timestamp. Interpret the
// demonstrated relative dates in local time; fall back for unknown/ambiguous dates.
fn parse_created(value: &str, fetched_at: DateTime<Local>) -> DateTime<Local> {
    let parsed = (|| {
        let (day, time) = value.split_once(" at ")?;
        let date = match day {
            "Today" => fetched_at.date_naive(),
            "Yesterday" => fetched_at.date_naive().checked_sub_days(Days::new(1))?,
            _ => return None,
        };
        let time = NaiveTime::parse_from_str(time, "%I:%M %P").ok()?;
        Local.from_local_datetime(&date.and_time(time)).single()
    })();
    parsed.unwrap_or(fetched_at)
}
