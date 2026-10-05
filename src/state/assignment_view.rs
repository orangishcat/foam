use std::io::{self, Error};

use chrono::{DateTime, Local, Utc};
use rusqlite::params;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    AppWindow, AssignmentFile, AssignmentUi, AssignmentWithDetails, Screen, SubmissionRow, UiState,
    database,
    state::top_bar::refresh_file_view,
    types::{assignment::Assignment, attachment::Attachments},
};

fn date(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Local)
        .format("%b %-d, %Y %-I:%M %p")
        .to_string()
}

fn plain_text(html: &str) -> String {
    if !html.contains('<') {
        return html.trim().to_owned();
    }
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn file_size(bytes: i64) -> String {
    let bytes = bytes as f64;
    let (size, unit) = if bytes >= 1024.0 * 1024.0 * 1024.0 {
        (bytes / (1024.0 * 1024.0 * 1024.0), "GB")
    } else if bytes >= 1024.0 * 1024.0 {
        (bytes / (1024.0 * 1024.0), "MB")
    } else {
        (bytes / 1024.0, "KB")
    };
    format!("{size:.1} {unit}")
}

fn files(attachments: &Attachments) -> ModelRc<AssignmentFile> {
    ModelRc::new(VecModel::from(
        attachments
            .files
            .file
            .iter()
            .map(|file| AssignmentFile {
                title: file.title.as_str().if_empty(&file.filename).into(),
                details: (&if file.filesize > 0 {
                    file_size(file.filesize)
                } else {
                    String::new()
                })
                    .into(),
                url: file.download_path.clone().into(),
                extension: if file.extension.is_empty() {
                    std::path::Path::new(&file.filename)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or_default()
                        .into()
                } else {
                    file.extension.as_str().into()
                },
            })
            .collect::<Vec<_>>(),
    ))
}

trait IfEmpty {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str;
}
impl IfEmpty for str {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.is_empty() { fallback } else { self }
    }
}

fn load(course: &str, id: &str) -> io::Result<Option<AssignmentWithDetails>> {
    let rows = database::from_sql_map(
        "SELECT a.*, COALESCE(c.course_title, a.course_id) AS course_title
         FROM assignments a LEFT JOIN courses c ON c.course_id = a.course_id
         WHERE a.id = ?2 AND (a.course_id = ?1 OR EXISTS (
             SELECT 1 FROM json_each(c.aliases) WHERE value = ?1
         ))"
        .into(),
        params![course, id],
        |row| {
            Ok((
                serde_rusqlite::from_row::<Assignment>(row).map_err(Error::other)?,
                row.get::<_, String>("course_title").map_err(Error::other)?,
            ))
        },
    )?;
    Ok(rows.into_iter().next().map(|(assignment, course_name)| {
        let grade = match (&assignment.score, &assignment.letter_grade) {
            (Some(score), Some(letter)) => {
                format!("{score} / {} ({letter})", assignment.max_points)
            }
            (Some(score), None) => format!("{score} / {}", assignment.max_points),
            (None, Some(letter)) => letter.clone(),
            (None, None) => format!("- / {}", assignment.max_points),
        };
        AssignmentWithDetails {
            id: assignment.id.clone().into(),
            title: assignment.title.clone().into(),
            course_id: assignment.course_id.clone().into(),
            course_name: course_name.into(),
            description: plain_text(&assignment.description).into(),
            due: date(assignment.due).into(),
            completion: (if assignment.is_completed() {
                "Completed"
            } else if assignment.is_past_due() {
                "Overdue"
            } else {
                "To do"
            })
            .into(),
            grade: grade.into(),
            submission_policy: assignment.allow_submissions,
            attachments: files(&assignment.attachments),
            submission_file_count: assignment
                .submissions
                .iter()
                .map(|submission| submission.attachments.files.file.len() as i32)
                .sum(),
            submissions: ModelRc::new(VecModel::from(
                assignment
                    .submissions
                    .iter()
                    .map(|submission| SubmissionRow {
                        id: submission.id.clone().into(),
                        user_id: submission.user_id.clone().into(),
                        created_at: date(submission.created).into(),
                        status: (if submission.draft {
                            "Draft"
                        } else if submission.late {
                            "Late"
                        } else {
                            "Submitted"
                        })
                        .into(),
                        body: plain_text(&submission.body).into(),
                        files: files(&submission.attachments),
                    })
                    .collect::<Vec<_>>(),
            )),
        }
    }))
}

pub fn init(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.global::<AssignmentUi>()
        .on_view_submissions(move |index| {
            if let Some(ui) = weak.upgrade() {
                crate::state::attachment_view::open_submissions(&ui, index);
            }
        });
    let weak = ui.as_weak();
    ui.global::<AssignmentUi>()
        .on_view_attachments(move |index| {
            if let Some(ui) = weak.upgrade() {
                crate::state::attachment_view::open(&ui, index);
            }
        });
    let weak = ui.as_weak();
    ui.global::<AssignmentUi>().on_open_browser(move || {
        if let Some(ui) = weak.upgrade() {
            let assignment = ui.global::<AssignmentUi>().get_assignment();
            if !assignment.id.is_empty() {
                crate::state::attachment_view::open_browser(&format!(
                    "https://{}.schoology.com/assignment/{}",
                    crate::config::config().subdomain.trim(),
                    assignment.id
                ));
            }
        }
    });
    let weak = ui.as_weak();
    ui.global::<AssignmentUi>().on_open(move |course, id| {
        let Some(ui) = weak.upgrade() else { return };
        let global = ui.global::<AssignmentUi>();
        global.set_error("".into());
        match load(&course, &id) {
            Ok(Some(assignment)) => {
                match crate::state::courses::show_assignment(&ui, &assignment.course_id, &id) {
                    Ok(()) => {
                        global.set_assignment(assignment);
                        ui.global::<UiState>().set_screen(Screen::Assignment);
                        refresh_file_view(&ui);
                    }
                    Err(err) => {
                        log::warn!("Opening assignment {id} failed: {err}");
                        global.set_error(format!("Could not open assignment: {err}").into());
                    }
                }
            }
            Ok(None) => {
                global.set_assignment(AssignmentWithDetails {
                    title: "Assignment".into(),
                    ..Default::default()
                });
                global.set_error(
                    format!("Assignment no longer exists: id={id} with course={course}").into(),
                );
                log::warn!("Assignment no longer exists:  id={id} with course={course}");
            }
            Err(err) => {
                log::warn!("Loading assignment {id} failed: {err}");
                global.set_assignment(AssignmentWithDetails {
                    title: "Assignment".into(),
                    ..Default::default()
                });
                global.set_error(format!("Could not load assignment: {err}").into());
            }
        }
    });
}
