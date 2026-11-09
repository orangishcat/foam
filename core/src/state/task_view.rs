use std::io::Error;

use chrono::{DateTime, Local, Utc};
use rusqlite::params;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::state::state::State;
use crate::{
    AppWindow, TaskFile, TaskUi, TaskWithDetails, FoldersUi, Screen,
    ScreenMenuItem, UiState, database,
    state::{state::menu_item, top_bar::refresh_file_view},
    types::{attachment::Attachments, task::Task},
};
use anyhow;

pub(crate) fn task_menu_items(ui: &AppWindow) -> Vec<ScreenMenuItem> {
    let task = ui.global::<TaskUi>().get_task();
    let folders_ui = ui.global::<FoldersUi>();
    let active = folders_ui.get_active_tab();
    let has_tab = active >= 0 && (active as usize) < folders_ui.get_tabs().row_count();
    let has_task = !task.id.is_empty();
    vec![
        menu_item("Close tab", &["Control", "W"], has_tab),
        menu_item("Copy details", &["Shift", "C"], has_task),
        menu_item(
            "View attachments",
            &["A"],
            task.attachments.row_count() > 0,
        ),
        menu_item("Open in browser", &["O"], has_task),
    ]
}

impl State for TaskViewState {
    fn sync_ui(&self, ui: &AppWindow) -> anyhow::Result<()> {
        let global = ui.global::<TaskUi>();
        let current = global.get_task();
        if !current.id.is_empty() {
            if let Some(task) = load(&current.folder_id, &current.id)? {
                global.set_task(task);
                global.set_error("".into());
            } else {
                global.set_error(format!("Task no longer exists: {}", current.id).into());
            }
            refresh_file_view(ui);
        }
        Ok(())
    }
    fn menu_items(&self, ui: &AppWindow) -> Vec<ScreenMenuItem> {
        task_menu_items(ui)
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

fn files(attachments: &Attachments) -> ModelRc<TaskFile> {
    ModelRc::new(VecModel::from(
        attachments
            .files
            .file
            .iter()
            .map(|file| TaskFile {
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

fn load(folder: &str, id: &str) -> anyhow::Result<Option<TaskWithDetails>> {
    let rows = database::from_sql_map(
        "SELECT a.*, COALESCE(f.title, a.folder_id) AS folder_title
         FROM tasks a LEFT JOIN folders f ON f.folder_id = a.folder_id
         WHERE a.id = ?2 AND (a.folder_id = ?1 OR EXISTS (
             SELECT 1 FROM json_each(f.data, '$.aliases') WHERE value = ?1
         ))"
        .into(),
        params![folder, id],
        |row| {
            Ok((
                serde_rusqlite::from_row::<Task>(row)?,
                row.get::<_, String>("folder_title")?,
            ))
        },
    )?;
    Ok(rows.into_iter().next().map(|(task, folder_name)| {
        TaskWithDetails {
            id: task.id.clone().into(),
            title: task.title.clone().into(),
            folder_id: task.folder_id.clone().into(),
            folder_name: folder_name.into(),
            description: plain_text(&task.description).into(),
            due: date(task.due).into(),
            completion: (if task.is_past_due() {
                "Overdue"
            } else {
                "To do"
            })
            .into(),
            attachments: files(&task.attachments),
            plugin_details: ModelRc::new(VecModel::from(plugin_details(&task.id))).into(),
        }
    }))
}

#[derive(Clone, Default)]
pub struct TaskViewState {}

impl TaskViewState {
    pub fn init_ui(&self, ui: &AppWindow) {
        let g = ui.global::<TaskUi>();
        let weak = ui.as_weak();
        g.on_view_attachments(move |index| {
            if let Some(ui) = weak.upgrade() {
                crate::state::attachment_view::open(&ui, index);
            }
        });
        let weak = ui.as_weak();
        g.on_open_browser(move |task_id| {
            if let Some(_ui) = weak.upgrade() {
                let account = match crate::account::active_account() {
                    Ok(account) => account,
                    Err(error) => {
                        log::warn!("Getting active account failed: {error}");
                        return;
                    }
                };
                crate::state::attachment_view::open_browser(&account.task_url(&task_id));
            }
        });
        let weak = ui.as_weak();
        g.on_open(move |folder, id| {
            let Some(ui) = weak.upgrade() else { return };
            let global = ui.global::<TaskUi>();
            global.set_error("".into());
            match load(&folder, &id) {
                Ok(Some(task)) => {
                    match crate::state::folders::FolderState::show_task(
                        &ui,
                        &task.folder_id,
                        &id,
                    ) {
                        Ok(()) => {
                            global.set_task(task);
                            ui.global::<UiState>().set_screen(Screen::Task);
                            refresh_file_view(&ui);
                        }
                        Err(err) => {
                            log::warn!("Opening task {id} failed: {err}");
                            global.set_error(format!("Could not open task: {err}").into());
                        }
                    }
                }
                Ok(None) => {
                    global.set_task(TaskWithDetails {
                        title: "Task".into(),
                        ..Default::default()
                    });
                    global.set_error(
                        format!("Task no longer exists: id={id} with folder={folder}").into(),
                    );
                    log::warn!("Task no longer exists:  id={id} with folder={folder}");
                }
                Err(err) => {
                    log::warn!("Loading task {id} failed: {err}");
                    global.set_task(TaskWithDetails {
                        title: "Task".into(),
                        ..Default::default()
                    });
                    global.set_error(format!("Could not load task: {err}").into());
                }
            }
        });
        g.on_copy_details(|task_id| {
            match copy_details(&task_id) {
                Ok(()) => log::debug!("Copied details for {task_id} to clipboard"),
                Err(err) => {
                    log::warn!("Failed to copy details for {task_id} to clipboard: {err}")
                }
            };
        });
    }
}

/// Parses the in-memory detail entries plugins pushed for a task
/// (`task_detail_put`) into renderable rows. Accepted JSON, either shape:
///
/// - `[{"label": "Grade", "value": "95%"}, …]`
/// - `{"fields": [{"label", "value"}, …], "render": "table"}`
///
/// With `render: "table"`, a value may carry multiple rows separated by
/// newlines with `|` as the cell separator; the view renders it verbatim.
fn plugin_details(task_id: &str) -> Vec<crate::TaskPluginDetail> {
    crate::plugin::host::task_details(task_id)
        .into_iter()
        .filter_map(|(plugin, json)| {
            let value = serde_json::from_str::<serde_json::Value>(&json).ok()?;
            let (fields, is_table) = match value {
                serde_json::Value::Array(fields) => (Some(fields), false),
                ref object => (
                    object.get("fields").and_then(|f| f.as_array().cloned()),
                    object.get("render").and_then(|r| r.as_str()) == Some("table"),
                ),
            };
            let fields = fields?;
            Some(
                fields
                    .into_iter()
                    .map(|field| {
                        let label = field.get("label").and_then(serde_json::Value::as_str);
                        let value = field.get("value").and_then(serde_json::Value::as_str);
                        crate::TaskPluginDetail {
                            plugin: plugin.clone().into(),
                            label: (label.unwrap_or_default()).into(),
                            value: (value.unwrap_or_default()).into(),
                            is_table,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

fn copy_details(task_id: &str) -> anyhow::Result<()> {
    let tasks = database::from_sql::<Task>(
        "SELECT * FROM tasks WHERE id = ?".to_owned(),
        &[&task_id.to_string()],
    )?;
    let mut clipboard = arboard::Clipboard::new()?;
    if tasks.is_empty() {
        return Err(Error::other("No task found with id = {task_id}").into());
    }
    if tasks.len() > 1 {
        log::warn!(
            "copy_details found {} entries with id {task_id}",
            tasks.len()
        );
    }
    let a = &tasks[0];
    Ok(clipboard
        .set_text(format!(
            "Task id: {task_id}\nTask title: {}\nFolder id: {}\nInternal database path: {}",
            a.title,
            a.folder_id,
            database::db_path()
                .to_str()
                .unwrap_or("Failed to get internal database path")
        ))?)
}
