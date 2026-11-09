use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{PickFirst, Same, json::JsonString, serde_as};

use crate::types::attachment::Attachments;
use anyhow;

#[serde_as]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    pub folder_id: String,
    pub id: String,
    pub source: String, // populated with 'builtin' when empty on write
    pub title: String,
    pub description: String,
    pub due: DateTime<Utc>,
    #[serde_as(as = "PickFirst<(JsonString, Same)>")]
    pub attachments: Attachments,
}

impl Task {
    pub(crate) fn insert_on(
        &self,
        connection: &rusqlite::Connection,
        folder_id: &str,
    ) -> anyhow::Result<()> {
        let mut task = self.clone();
        if task.source.is_empty() {
            task.source = crate::plugin::host::attribution();
        }
        task.folder_id = folder_id.to_owned();
        let serialized = serde_rusqlite::to_params_named(&task)?;
        connection.execute(
            include_str!("../sql/task/write.sql"),
            serialized.to_slice().as_slice(),
        )?;
        Ok(())
    }
    pub fn is_past_due(&self) -> bool {
        self.due < Utc::now()
    }
}
