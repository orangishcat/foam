// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod config;
mod request;
mod state;

use state::State;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let app_config = config::AppConfig::load(None);
    request::init(&app_config)?;
    let state = State::new(app_config);

    let ui = Home::new()?;
    ui.run()?;

    Ok(())
}
