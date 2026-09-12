use crate::{
    api::schoology::RequestResult,
    types::notification::{Notification, NotificationEvent},
};

mod home;
mod material_type;
mod navigation;
mod update;

/// Combine material posts from the full HTML feed with grades from the navigation feed.
/// Both feeds must succeed to avoid returning an incomplete notification history.
pub fn scrape_notifications() -> RequestResult<Vec<Notification>> {
    let mut notifications = home::scrape_home()?;
    notifications.retain(|item| item.event == NotificationEvent::MaterialPosted);
    notifications.extend(navigation::scrape_notifications()?);
    notifications.sort_by_key(|item| std::cmp::Reverse(item.created));
    Ok(notifications)
}
