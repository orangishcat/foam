use std::{collections::HashSet, io};

use super::super::{RequestResult, internal_get};
use crate::{
    api::schoology::{notification::navigation, types::notification::HomeResponse},
    types::notification::Notification,
};
use chrono::{DateTime, Days, Local, NaiveDate, NaiveTime, TimeZone};
use scraper::{CaseSensitivity, ElementRef, Html, Selector};

const HOME_ROUTE: &str = "/home/notifications?filter=all";

pub fn scrape_home() -> RequestResult<Vec<Notification>> {
    let mut route = HOME_ROUTE.to_owned();
    let mut visited = HashSet::new();
    let mut notifications = Vec::new();
    let now = Local::now();

    loop {
        if !visited.insert(route.clone()) {
            return Err(io::Error::other("notification pagination cycle").into());
        }
        let response: HomeResponse = internal_get(&route)?;
        let (page, next) = parse_home(&response.output, now)?;
        notifications.extend(page);
        match next {
            Some(next) => route = next,
            None => return Ok(notifications),
        }
    }
}

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("static notification selector")
}

fn text(element: ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns the resource and any course encoded in the link.
fn resource(href: &str) -> Option<(String, String)> {
    let url = reqwest::Url::parse("https://schoology.com")
        .ok()?
        .join(href)
        .ok()?;
    let parts: Vec<_> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let numeric = |id: &str| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit());

    match parts.as_slice() {
        ["course", course, "materials", rest @ ..] => {
            let id = rest.iter().rev().find(|id| numeric(id))?;
            Some(((*id).to_owned(), (*course).to_owned()))
        }
        [
            "assignment" | "assessment" | "discussion" | "event" | "page",
            id,
            ..,
        ] if numeric(id) => Some(((*id).to_owned(), String::new())),
        _ => None,
    }
}

// Capture the actor and operation preceding the first resource. Hidden icon
// labels and timestamps are presentation details rather than title content.
fn operation_prefix(element: ElementRef<'_>, output: &mut String) -> bool {
    if element
        .value()
        .classes()
        .any(|class| matches!(class, "visually-hidden" | "edge-time" | "other-items-link"))
    {
        return false;
    }
    if element.attr("href").and_then(resource).is_some() {
        return true;
    }
    for child in element.children() {
        if let Some(child) = ElementRef::wrap(child) {
            if operation_prefix(child, output) {
                return true;
            }
        } else if let Some(value) = child.value().as_text() {
            output.push_str(value);
        }
    }
    false
}

fn parse_home(
    html: &str,
    now: DateTime<Local>,
) -> RequestResult<(Vec<Notification>, Option<String>)> {
    let document = Html::parse_fragment(html);
    let feed = document
        .select(&selector("ul.s-notifications-mini"))
        .next()
        .ok_or_else(|| io::Error::other("notification HTML is missing its feed list"))?;
    let links = selector("a[href]");
    let sentences = selector(".edge-sentence");
    let times = selector(".edge-time");
    let material_items = selector(".material-item");
    let material_times = selector(".material-created");
    let mut date = None;
    let mut notifications = Vec::new();
    let mut next = None;

    for row in feed.child_elements() {
        if row
            .value()
            .has_class("notif-date-header", CaseSensitivity::CaseSensitive)
        {
            date = NaiveDate::parse_from_str(&text(row), "%A, %B %d, %Y").ok();
            continue;
        }
        if row
            .value()
            .has_class("notif-more", CaseSensitivity::CaseSensitive)
        {
            if let Some(link) = row.select(&links).next() {
                let url = reqwest::Url::parse("https://schoology.com")?
                    .join(link.attr("href").unwrap_or_default())?;
                let page = url
                    .query_pairs()
                    .find(|(key, _)| key == "page")
                    .and_then(|(_, value)| value.parse::<usize>().ok())
                    .ok_or_else(|| io::Error::other("invalid notification pagination link"))?;
                next = Some(format!("{HOME_ROUTE}&page={page}"));
            }
            continue;
        }

        let sentence = row
            .select(&sentences)
            .next()
            .ok_or_else(|| io::Error::other("notification row is missing its sentence"))?;
        let created = sentence
            .select(&times)
            .next()
            .map(|time| parse_created(&text(time), date, now))
            .unwrap_or(now);
        let course_id = sentence
            .select(&links)
            .find_map(|link| {
                let id = link.attr("href")?.strip_prefix("/course/")?;
                (!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
                    .then(|| id.to_owned())
            })
            .unwrap_or_default();
        // The observed HTML has no read-state attribute. Recognize common
        // unread markers if Schoology supplies one; otherwise treat it as viewed.
        let viewed = !row.descendent_elements().any(|element| {
            element
                .value()
                .classes()
                .any(|class| matches!(class, "unread" | "unviewed" | "notif-unread"))
        });
        let mut prefix = String::new();
        operation_prefix(sentence, &mut prefix);
        let prefix = prefix.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut seen = HashSet::new();

        // Expanded item lists repeat the preview links, so deduplicate resources
        // only within each source row.
        for link in row.select(&links) {
            let Some((resource_id, linked_course)) = link.attr("href").and_then(resource) else {
                continue;
            };
            if !seen.insert((resource_id.clone(), linked_course.clone())) {
                continue;
            }
            let item_created = link
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|ancestor| ancestor.select(&material_items).next().is_some())
                .and_then(|item| item.select(&material_times).next())
                .and_then(|time| parse_posted_time(&text(time), date))
                .unwrap_or(created);
            notifications.push(Notification {
                title: format!("{prefix} {}", text(link)).trim().to_owned(),
                viewed,
                created: item_created,
                resource_id,
                course_id: if linked_course.is_empty() {
                    course_id.clone()
                } else {
                    linked_course
                },
            });
        }
        if seen.is_empty() {
            notifications.push(Notification {
                title: prefix,
                viewed,
                created,
                resource_id: String::new(),
                course_id,
            });
        }
    }

    Ok((notifications, next))
}

fn parse_created(value: &str, header: Option<NaiveDate>, now: DateTime<Local>) -> DateTime<Local> {
    let parsed = (|| {
        let (day, time) = value.split_once(" at ")?;
        let date = match day {
            "Today" => now.date_naive(),
            "Yesterday" => now.date_naive().checked_sub_days(Days::new(1))?,
            _ => header?,
        };
        let time = NaiveTime::parse_from_str(time, "%I:%M %P").ok()?;
        Local.from_local_datetime(&date.and_time(time)).single()
    })();
    parsed.unwrap_or(now)
}

fn parse_posted_time(value: &str, date: Option<NaiveDate>) -> Option<DateTime<Local>> {
    let time = value.strip_prefix("Posted on ")?;
    let time = NaiveTime::parse_from_str(time, "%I:%M %P").ok()?;
    Local.from_local_datetime(&date?.and_time(time)).single()
}
