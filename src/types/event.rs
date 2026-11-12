use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{Same, serde_as};

use anyhow;

#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub description: String,
    pub due: DateTime<Utc>,
    /// "schoology" for events scraped from the calendar feed, "user" for user-created events.
    #[serde_as(as = "Same")]
    pub source: String,
}

impl Event {
    pub(crate) fn insert_on(&self, connection: &rusqlite::Connection) -> anyhow::Result<()> {
        let serialized = serde_rusqlite::to_params_named(self)?;
        connection.execute(
            include_str!("../sql/event/write.sql"),
            serialized.to_slice().as_slice(),
        )?;
        Ok(())
    }
}
