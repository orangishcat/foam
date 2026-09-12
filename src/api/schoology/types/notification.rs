use serde::Deserialize;

use crate::api::types::LooseString;

#[derive(Deserialize)]
pub(crate) struct HomeResponse {
    pub output: String,
}

#[derive(Deserialize)]
pub(crate) struct NotificationsResponse {
    pub data: Vec<SchoologyNotification>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchoologyNotification {
    pub sentence: String,
    pub viewed: bool,
    pub created: String,
    pub realm: String,
    pub realm_id: LooseString,
    pub args: Vec<NotificationArgument>,
}

#[derive(Deserialize)]
pub(crate) struct NotificationArgument {
    pub id: LooseString,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
}

impl NotificationArgument {
    pub(crate) fn is_context(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "s_content_course_section" | "s_user" | "s_user_user" | "s_content_group"
        )
    }
}
