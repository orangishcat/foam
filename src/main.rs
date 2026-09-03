// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use crate::{config::config, state::AppState};

mod config;
mod filesystem;
mod schoology;
mod state;
mod types;

slint::include_modules!();

/// Scrapes the user's course sections and each section's material tree.
///
/// Schoology API: <https://developers.schoology.com/api-documentation/rest-api-v1/course-section/>
fn scrape_courses() -> Result<(), Box<dyn Error + Send + Sync>> {
    let courses = schoology::course::courses::scrape_courses()?;
    filesystem::write_courses(&courses)?;
    Ok(())
}

fn init(state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    state.load_courses();
    Ok(())
}

fn shutdown() -> Result<(), Box<dyn std::error::Error>> {
    config().save()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut state = AppState::default();
    init(&mut state)?;

    let ui = AppWindow::new()?;
    ui.run()?;
    shutdown()?;

    Ok(())
}
