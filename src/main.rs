// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use crate::config::config;

mod config;
mod filesystem;
mod schoology;
mod types;

slint::include_modules!();

/// Scrapes the user's course sections and each section's material tree.
///
/// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/course-section/>
fn scrape_courses() -> Result<(), Box<dyn Error + Send + Sync>> {
    let courses = schoology::course::courses::courses()?;
    let destination = config().data_dir().join("courses");
    filesystem::write_courses(&courses, &destination)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let configured = {
        let config = config();
        !config.subdomain.is_empty() || config.api_key.is_some()
    };
    if configured {
        scrape_courses().map_err(|error| -> Box<dyn Error> { error })?;
    }

    let ui = Home::new()?;
    ui.run()?;

    Ok(())
}
