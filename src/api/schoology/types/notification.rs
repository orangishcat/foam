use serde::Deserialize;

use crate::api::types::LooseString;

#[derive(Deserialize)]
pub(crate) struct NotificationsResponse {
    pub data: Vec<SchoologyNotification>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchoologyNotification {
    #[serde(rename = "type", default)]
    pub kind: String,
    pub viewed: bool,
    pub created: String,
    pub args: Vec<NotificationArgument>,
}

#[derive(Deserialize)]
pub(crate) struct NotificationArgument {
    pub id: LooseString,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub document_type: String,
}
