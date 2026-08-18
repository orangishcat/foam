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
    ui.set_counter(state.config().counter.into());

    let ui_handle = ui.as_weak();
    ui.on_request_increase_value(move || {
        let ui = ui_handle.unwrap();
        ui.set_counter(ui.get_counter() + 1);
    });

    ui.run()?;

    Ok(())
}
