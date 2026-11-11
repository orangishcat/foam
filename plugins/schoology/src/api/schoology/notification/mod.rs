mod navigation;
pub mod update;

/// Schoology-owned notification event names. Foam stores these as opaque
/// source values and does not encode Schoology-specific variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchoologyNotificationType {
    ResourcePosted,
    GradeUpdated,
}

impl SchoologyNotificationType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourcePosted => "resource_posted",
            Self::GradeUpdated => "grade_updated",
        }
    }
}

use foam::{
    database,
    types::{notification::Notification, resource::ResourceType},
};
use slint::{ComponentHandle, ToSharedString};

/// Open a notification using Schoology's resource type and IDs, rather than
/// asking core to infer behavior from a Schoology-specific event label.
pub fn open_notification(ui: &foam::AppWindow, id: &str) -> bool {
    let notification = match database::from_sql::<Notification>(
        "SELECT * FROM notifications WHERE id = ?".into(),
        &[&id],
    ) {
        Ok(mut rows) => match rows.pop() {
            Some(notification) => notification,
            None => return false,
        },
        Err(error) => {
            log::warn!("Loading Schoology notification {id} failed: {error}");
            return false;
        }
    };
    let folder_id = match resolve_notification_folder(&notification) {
        Ok(Some(folder_id)) => folder_id,
        Ok(None) => {
            log::warn!("Schoology notification {id} has no resolvable folder");
            return false;
        }
        Err(error) => {
            log::warn!("Resolving Schoology notification {id} folder failed: {error}");
            return false;
        }
    };
    let resource_id = notification.resource_id;
    let resource_type = database::from_sql_map(
        "SELECT type FROM resources WHERE folder_id = ?1 AND resource_id = ?2".into(),
        &[&folder_id, &resource_id],
        |row| Ok(row.get::<_, String>(0)?),
    )
    .ok()
    .and_then(|mut rows| rows.drain(..).next())
    .and_then(|kind| serde_json::from_value::<ResourceType>(serde_json::Value::String(kind)).ok())
    .or(notification.resource_type);
    match resource_type {
        Some(ResourceType::Task | ResourceType::Assessment) => {
            ui.global::<foam::TaskUi>()
                .invoke_open(folder_id.to_shared_string(), resource_id.to_shared_string());
            true
        }
        Some(ResourceType::Document) => {
            ui.global::<foam::DocumentUi>()
                .invoke_open(folder_id.to_shared_string(), resource_id.to_shared_string());
            true
        }
        Some(ResourceType::Folder) => {
            ui.global::<foam::FoldersUi>()
                .invoke_open(folder_id.to_shared_string());
            ui.global::<foam::FoldersUi>()
                .invoke_open_resource_folder(folder_id.to_shared_string(), resource_id.to_shared_string());
            true
        }
        Some(ResourceType::Link) => {
            match database::from_sql_map(
                "SELECT json_extract(data, '$.resource.url') FROM resources WHERE folder_id = ? AND resource_id = ? AND type = 'link'".into(),
                &[&folder_id, &resource_id],
                |row| Ok(row.get::<_, String>(0)?),
            ) {
                Ok(urls) => {
                    if let Some(url) = urls.first().filter(|url| !url.is_empty()) {
                        foam::state::attachment_view::open_browser(url);
                        true
                    } else {
                        false
                    }
                }
                Err(error) => {
                    log::warn!("Loading Schoology notification link {id} failed: {error}");
                    false
                }
            }
        }
        None => {
            log::warn!("Schoology notification {id} has no resource type");
            false
        }
    }
}

fn resolve_notification_folder(notification: &Notification) -> anyhow::Result<Option<String>> {
    if let Some(folder_id) = notification.folder_id.as_deref().filter(|id| !id.is_empty()) {
        if let Some(folder) = foam::types::folder_info::folder(folder_id)? {
            return Ok(Some(folder.folder_id));
        }
        // Schoology sometimes attaches a stale section id to a notification,
        // while its title still identifies the course that owns the resource.
        let title = notification
            .folder_title
            .split_once(": Section ")
            .map_or(notification.folder_title.as_str(), |(title, _)| title);
        let matches = database::from_sql_map(
            "SELECT material_id FROM plugin_schoology_materials
             WHERE kind = 'course' AND course_title = ?".into(),
            &[&title],
            |row| Ok(row.get::<_, String>(0)?),
        )?;
        if matches.len() == 1 {
            return Ok(matches.into_iter().next());
        }
    }

    // Grade notifications may omit section identity entirely; the plugin's
    // custom assignment table maps their item ID back to a unique course.
    let matches = database::from_sql_map(
        "SELECT DISTINCT folder_id FROM plugin_schoology_assignments WHERE id = ?".into(),
        &[&notification.resource_id],
        |row| Ok(row.get::<_, String>(0)?),
    )?;
    Ok((matches.len() == 1).then(|| matches[0].clone()))
}
