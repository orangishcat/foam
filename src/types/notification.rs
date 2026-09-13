use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::material::MaterialType;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    MaterialPosted,
    GradeUpdated,
    #[default]
    Unknown,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub event: NotificationEvent,
    pub title: String,
    pub viewed: bool,
    pub is_processed: bool, // whether foam has processed this notification by updating syncing
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    pub material_type: Option<MaterialType>,
    pub course_id: String,
}
