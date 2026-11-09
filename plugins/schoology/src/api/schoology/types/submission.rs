use serde::{Deserialize, Serialize};

use crate::api::schoology::course::materials::types::Attachments;
use crate::api::types::{LooseInt, LooseString};

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
    pub body: String,
    #[serde(default)]
    pub attachments: Attachments,
}
