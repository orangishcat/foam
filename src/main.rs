// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

mod config;
mod request;
slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let configured = {
        let config = config::CONFIG.read().expect("config lock is poisoned");
        !config.subdomain.is_empty() || config.api_key.is_some()
    };
    if configured {
        request::schoology::courses::courses();
    }

    let ui = Home::new()?;
    ui.run()?;

    Ok(())
}
