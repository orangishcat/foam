// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod config;
mod request;
mod state;

use state::State;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let mut app_config = config::AppConfig::load(None);
    if !app_config.subdomain.is_empty() || app_config.api_key.is_some() {
        request::courses(&mut app_config)?;
    }
    let state = State::new(app_config);

    let ui = Home::new()?;
    ui.run()?;

    Ok(())
}
