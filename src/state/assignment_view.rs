use std::io::Error;

use chrono::{DateTime, Local, Utc};
use rusqlite::params;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::state::state::State;
use crate::{
    AppWindow, AssignmentFile, AssignmentUi, AssignmentWithDetails, CoursesUi, Screen,
    ScreenMenuItem, SubmissionRow, UiState, database,
    state::{state::menu_item, top_bar::refresh_file_view},
    types::{assignment::Assignment, attachment::Attachments},
};
use anyhow;

pub(crate) fn assignment_menu_items(ui: &AppWindow) -> Vec<ScreenMenuItem> {
    let assignment = ui.global::<AssignmentUi>().get_assignment();
    let courses_ui = ui.global::<CoursesUi>();
    let active = courses_ui.get_active_tab();
    let has_tab = active >= 0 && (active as usize) < courses_ui.get_tabs().row_count();
    let has_assignment = !assignment.id.is_empty();
    vec![
        menu_item("Close tab", &["Control", "W"], has_tab),
        menu_item("Copy details", &["Shift", "C"], has_assignment),
        menu_item(
            "View attachments",
            &["A"],
            assignment.attachments.row_count() > 0,
        ),
        menu_item("Open in browser", &["O"], has_assignment),
        menu_item(
            "View submissions",
            &["S"],
            assignment.submission_file_count > 0,
        ),
    ]
}

impl State for AssignmentViewState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        let global = ui.global::<AssignmentUi>();
        let current = global.get_assignment();
        if !current.id.is_empty() {
            if let Some(assignment) = load(&current.course_id, &current.id)? {
                global.set_assignment(assignment);
                global.set_error("".into());
            } else {
                global.set_error(format!("Assignment no longer exists: {}", current.id).into());
            }
            refresh_file_view(ui);
        }
        Ok(())
    }
    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        assignment_menu_items(ui)
    }
}

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

fn load(course: &str, id: &str) -> anyhow::Result<Option<AssignmentWithDetails>> {
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
                serde_rusqlite::from_row::<Assignment>(row)?,
                row.get::<_, String>("course_title")?,
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

#[derive(Clone, Default)]
pub struct AssignmentViewState {}

impl AssignmentViewState {
    pub fn init_ui(&self, ui: &AppWindow) {
        let g = ui.global::<AssignmentUi>();
        let weak = ui.as_weak();
        g.on_view_submissions(move |index| {
            if let Some(ui) = weak.upgrade() {
                crate::state::attachment_view::open_submissions(&ui, index);
            }
        });
        let weak = ui.as_weak();
        g.on_view_attachments(move |index| {
            if let Some(ui) = weak.upgrade() {
                crate::state::attachment_view::open(&ui, index);
            }
        });
        let weak = ui.as_weak();
        g.on_open_browser(move |assignment_id| {
            if let Some(_ui) = weak.upgrade() {
                let account = match crate::account::active_account() {
                    Ok(account) => account,
                    Err(error) => {
                        log::warn!("Getting active account failed: {error}");
                        return;
                    }
                };
                crate::state::attachment_view::open_browser(
                    &account.assignment_url(&assignment_id),
                );
            }
        });
        let weak = ui.as_weak();
        g.on_open(move |course, id| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<AssignmentUi>();
            global.set_error("".into());
            match load(&course, &id) {
                Ok(Some(assignment)) => {
                    match crate::state::courses::CourseState::show_assignment(
                        &ui,
                        &assignment.course_id,
                        &id,
                    ) {
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
        g.on_copy_details(|assignment_id| {
            match copy_details(&assignment_id) {
                Ok(()) => log::debug!("Copied details for {assignment_id} to clipboard"),
                Err(err) => {
                    log::warn!("Failed to copy details for {assignment_id} to clipboard: {err}")
                }
            };
        });
    }
}

fn copy_details(assignment_id: &str) -> anyhow::Result<()> {
    let assignments = database::from_sql::<Assignment>(
        "SELECT * FROM assignments WHERE id = ?".to_owned(),
        &[&assignment_id.to_string()],
    )?;
    let mut clipboard = arboard::Clipboard::new()?;
    if assignments.is_empty() {
        return Err(Error::other("No assignment found with id = {assignment_id}").into());
    }
    if assignments.len() > 1 {
        log::warn!(
            "copy_details found {} entries with id {assignment_id}",
            assignments.len()
        );
    }
    let a = &assignments[0];
    Ok(clipboard
        .set_text(format!(
            "Assignment id: {assignment_id}\nAssignment title: {}\nCourse id: {}\nInternal database path: {}",
            a.title,
            a.course_id,
            database::db_path()
                .to_str()
                .unwrap_or("Failed to get internal database path")
        ))?)
}
