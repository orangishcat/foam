//! Updates to cached assignments from Schoology's calendar export.
use std::{collections::HashMap, io, sync::LazyLock};

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use ical::{IcalParser, property::Property};
use itertools::Itertools;
use regex::Regex;
use rusqlite::{Params, ToSql, params, params_from_iter};
use serde_rusqlite::NamedParamSlice;

use super::RequestResult;
use crate::{
    config::config,
    database,
    types::{
        assignment::{self, Assignment},
        course::Course,
        material::Material,
    },
};

static ASSIGNMENT_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)https?://(?:[a-z0-9-]+\.)*schoology\.com/assignment/(\d+)(?:[/?#\s<>"']|$)"#)
        .unwrap()
});

/// The feed URL is a bearer credential; never include it in request errors.
pub fn fetch() -> RequestResult<(Vec<String>, Vec<CalendarAssignment>)> {
    let url = config().calendar_url.clone();
    if url.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
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

#[derive(Debug, PartialEq, Eq)]
pub struct CalendarAssignment {
    due: DateTime<Utc>,
    title: Option<String>,
    description: Option<String>,
}

fn unescape(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => result.push('\n'),
                Some(c) => result.push(c),
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn parse(body: &str) -> RequestResult<(Vec<String>, Vec<CalendarAssignment>)> {
    let mut ids: Vec<String> = Vec::new();
    let mut assignments: Vec<CalendarAssignment> = Vec::new();
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
            ids.push(id);
            assignments.push(CalendarAssignment {
                due,
                title: value(props, "SUMMARY").map(unescape),
                description: value(props, "DESCRIPTION").map(unescape),
            });
        }
    }
    if !seen_calendar {
        return Err(io::Error::other("response does not contain an iCalendar calendar").into());
    }
    Ok((ids, assignments))
}

pub fn apply(ids: &Vec<String>, cal_assignments: &Vec<CalendarAssignment>) -> io::Result<usize> {
    let assignments = database::from_sql::<Assignment>(
        "SELECT * FROM assignments WHERE ID = (?, ?, ?)".to_owned(),
        &ids.iter()
            .map(|id| id as &dyn rusqlite::ToSql) // <-- convert to tosql borrow to fix type
            .collect::<Vec<_>>(),
    )?;

    let mut changed_assignments = vec![];
    for (i, (old, new)) in assignments
        .into_iter()
        .zip(cal_assignments.into_iter())
        .enumerate()
    {
        if old.id != ids[i] {
            return Err(io::Error::other(format!(
                "ids do not match: old={}, new={}",
                old.id, ids[i]
            )));
        }

        let changed = new.title.clone().is_some_and(|s| old.title != s)
            || new
                .description
                .clone()
                .is_some_and(|d| old.description != d)
            || old.due != new.due;
        if !changed {
            continue;
        }

        changed_assignments.push(Assignment {
            title: new.title.clone().unwrap_or(old.title),
            due: new.due,
            description: new.description.clone().unwrap_or(old.description),
            ..old
        });
    }
    if !changed_assignments.is_empty() {
        let serialized = changed_assignments
            .iter()
            .map(|assignment| {
                serde_rusqlite::to_params_named_with_fields(
                    assignment,
                    &["title", "due", "description", "id"],
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        let slices = serialized.iter().map(|p| p.to_slice()).collect::<Vec<_>>();
        database::bulk_execute(
            "UPDATE assignments SET title = :title, due = :due, description = :description WHERE id = :id"
                .to_owned(),
            slices.iter().map(|p| p.as_slice()).collect(),
        )?;
    }
    Ok(changed_assignments.len())
}
