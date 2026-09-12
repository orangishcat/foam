use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::material::MaterialType;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    MaterialPosted,
    GradeUpdated,
    /// The event is unknown, including notifications saved before event tracking.
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    #[serde(default)]
    pub event: NotificationEvent,
    pub title: String,
    pub viewed: bool,
    pub is_processed: bool, // whether foam has processed this notification by updating syncing
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    /// None for unsupported, unknown, or non-material resources.
    #[serde(default)]
    pub material_type: Option<MaterialType>,
    pub course_id: String,
}
