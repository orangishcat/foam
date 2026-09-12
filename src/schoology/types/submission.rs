use serde::{Deserialize, Serialize};

use crate::types::{LooseString, submission::Submission};

use crate::schoology::course::materials::types::{Attachments, LooseInt};

#[derive(Serialize, oauth::Request)]
pub(crate) struct SubmissionsQuery {
    pub with_attachments: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub(crate) struct SubmissionsResponse {
    pub revision: Vec<Revision>,
}

#[derive(Deserialize)]
pub(crate) struct Revision {
    pub revision_id: LooseString,
    pub uid: LooseString,
    pub created: LooseInt,
    #[serde(default)]
    pub late: LooseInt,
    #[serde(default)]
    pub draft: LooseInt,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub attachments: Attachments,
}

impl From<Revision> for Submission {
    fn from(revision: Revision) -> Self {
        Self {
            id: revision.revision_id.0,
            user_id: revision.uid.0,
            created: chrono::DateTime::from_timestamp(revision.created.0, 0).unwrap_or_default(),
            late: revision.late.0 != 0,
            draft: revision.draft.0 != 0,
            body: revision.body,
            attachments: revision.attachments.into(),
        }
    }
}
