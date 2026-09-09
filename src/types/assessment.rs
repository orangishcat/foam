use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Assessment {
    pub id: String,
    pub title: String,
    pub description: String,
    pub max_points: f64,
    pub due: DateTime<Utc>,
    pub completed: bool,
}
