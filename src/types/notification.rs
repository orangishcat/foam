use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub viewed: bool,
    pub created: DateTime<Local>, // best effort; if api doesn't expose use local time when fetching the resource
    pub resource_id: String,
    pub course_id: String,
}
