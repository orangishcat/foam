use crate::{
    api::schoology::{RequestResult, notification::home::scrape_home},
    types::notification::Notification,
};

mod home;
mod navigation;

/// Fetch notifications from the full HTML feed, falling back to the navigation
/// feed if the primary route or its HTML format fails.
pub fn scrape_notifications() -> RequestResult<Vec<Notification>> {
    home::scrape_home().or_else(|error| {
        log::warn!("Schoology notification feed failed; using fallback: {error}");
        navigation::scrape_notifications()
            .inspect_err(|e| log::warn!("Schoology iapi2 notification feed failed: {e}"))
    })
}
