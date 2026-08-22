use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assessment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub max_points: f64,
    #[serde(default)]
    pub due: DateTime<Utc>,
    #[serde(default)]
    pub completed: bool,
}
