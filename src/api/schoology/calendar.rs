//! Assignment due dates from Schoology's calendar export.
use std::{
    collections::{HashMap, HashSet},
    io,
    sync::LazyLock,
};

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use ical::{IcalParser, property::Property};
use regex::Regex;

use super::RequestResult;
use crate::{
    config::config,
    types::{course::Course, material::Material},
};

static ASSIGNMENT_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)https?://(?:[a-z0-9-]+\.)*schoology\.com/assignment/(\d+)(?:[/?#\s<>"']|$)"#)
        .unwrap()
});

/// The feed URL is a bearer credential; never include it in request errors.
pub fn fetch() -> RequestResult<HashMap<String, DateTime<Utc>>> {
    let url = config().calendar_url.clone();
    if url.trim().is_empty() {
        return Ok(HashMap::new());
    }
    crate::thread_manager::check_cancelled()?;
    let url = feed_url(&url)?;
    let body = super::new_client()?
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::text)
        .map_err(reqwest::Error::without_url)?;
    crate::thread_manager::check_cancelled()?;
    parse(&body)
}

fn feed_url(value: &str) -> RequestResult<reqwest::Url> {
    let value = value.trim();
    let value = if value
        .get(..9)
        .is_some_and(|s| s.eq_ignore_ascii_case("webcal://"))
    {
        format!("https://{}", &value[9..])
    } else {
        value.to_owned()
    };
    let url = reqwest::Url::parse(&value)?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(io::Error::other("calendar URL must use webcal, https, or http").into());
    }
    Ok(url)
}

fn property<'a>(properties: &'a [Property], name: &str) -> Option<&'a Property> {
    properties
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}

fn value<'a>(properties: &'a [Property], name: &str) -> Option<&'a str> {
    property(properties, name)?.value.as_deref()
}

fn due_date(start: &Property, default_timezone: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = start.value.as_deref()?;
    if let Some(raw) = raw.strip_suffix('Z') {
        return NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S")
            .ok()
            .map(|dt| dt.and_utc());
    }
    // Like schoology-ics, date-only entries begin at local midnight.
    let naive = if raw.len() == 8 {
        NaiveDate::parse_from_str(raw, "%Y%m%d")
            .ok()?
            .and_hms_opt(0, 0, 0)?
    } else {
        NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S").ok()?
    };
    let timezone = start
        .params
        .as_ref()
        .and_then(|params| {
            params
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("TZID"))
                .and_then(|(_, values)| values.first())
                .map(String::as_str)
        })
        .or(default_timezone);
    match timezone {
        Some(name) => name
            .trim_matches('"')
            .parse::<chrono_tz::Tz>()
            .ok()?
            .from_local_datetime(&naive)
            .single()
            .map(|dt| dt.with_timezone(&Utc)),
        None => Local
            .from_local_datetime(&naive)
            .single()
            .map(|dt| dt.with_timezone(&Utc)),
    }
}

fn parse(body: &str) -> RequestResult<HashMap<String, DateTime<Utc>>> {
    let mut dates = HashMap::new();
    let mut seen_calendar = false;
    for calendar in IcalParser::new(io::Cursor::new(body)) {
        let calendar = calendar.map_err(|_| io::Error::other("invalid iCalendar feed"))?;
        seen_calendar = true;
        let timezone = value(&calendar.properties, "X-WR-TIMEZONE");
        for event in calendar.events {
            let props = &event.properties;
            if value(props, "STATUS").is_some_and(|s| s.eq_ignore_ascii_case("CANCELLED"))
                || property(props, "RRULE").is_some()
                || property(props, "RECURRENCE-ID").is_some()
            {
                continue;
            }
            // Match IDs, never titles: multiple courses can reuse assignment names.
            let id = ["URL", "DESCRIPTION", "SUMMARY", "LOCATION"]
                .into_iter()
                .filter_map(|name| value(props, name))
                .find_map(|text| {
                    let text = text.replace("\\n", "\n").replace("\\N", "\n");
                    ASSIGNMENT_LINK.captures(&text).map(|c| c[1].to_owned())
                });
            let Some(id) = id else { continue };
            let Some(due) = property(props, "DTSTART").and_then(|p| due_date(p, timezone)) else {
                log::warn!("Skipping calendar assignment {id}: unsupported or invalid DTSTART");
                continue;
            };
            // Conflicting occurrences cannot be represented by a single assignment due date.
            dates
                .entry(id)
                .and_modify(|old| {
                    if *old != Some(due) {
                        *old = None;
                    }
                })
                .or_insert(Some(due));
        }
    }
    if !seen_calendar {
        return Err(io::Error::other("response does not contain an iCalendar calendar").into());
    }
    Ok(dates
        .into_iter()
        .filter_map(|(id, due)| due.map(|due| (id, due)))
        .collect())
}

pub fn apply(courses: &mut [Course], dates: &HashMap<String, DateTime<Utc>>) -> usize {
    let mut updated = 0;
    for course in courses {
        for material in course.materials.recursive_iter_mut() {
            if let Material::Assignment(assignment) = material
                && let Some(&due) = dates.get(&assignment.id)
                && assignment.due != due
            {
                assignment.due = due;
                updated += 1;
            }
        }
    }
    updated
}

fn missing_ids(courses: &[Course], dates: &HashMap<String, DateTime<Utc>>) -> Vec<String> {
    let existing: HashSet<&str> = courses
        .iter()
        .flat_map(|course| course.materials.recursive_iter())
        .filter_map(|material| match material {
            Material::Assignment(a) => Some(a.id.as_str()),
            _ => None,
        })
        .collect();
    let mut missing: Vec<_> = dates
        .keys()
        .filter(|id| !existing.contains(id.as_str()))
        .cloned()
        .collect();
    missing.sort();
    missing
}

/// Fetch outside the state lock. A failure affects only that calendar item.
pub fn fetch_missing(dates: &HashMap<String, DateTime<Utc>>) -> usize {
    let missing = missing_ids(&crate::state::state::state().course.courses, dates);
    let mut fetched = 0;
    for id in missing {
        if crate::thread_manager::check_cancelled().is_err() {
            break;
        }
        match super::notification::update::fetch_calendar_assignment(&id) {
            Ok(true) => fetched += 1,
            Ok(false) => {}
            Err(err) => log::warn!("Fetching calendar assignment {id} failed: {err}"),
        }
    }
    fetched
}
