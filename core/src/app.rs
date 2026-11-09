use std::error::Error;
use std::sync::OnceLock;

use slint::ComponentHandle;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

use crate::{
    AppWindow, config,
    logging,
    state::state::state as state_ctl,
    state::sidebar::Sidebar,
    ui::WEAK_UI,
};

/// Native account plugins, wired by the top-level binary via the
/// `features = ["schoology-plugin"]` dependency seam. Omitted entirely when
/// built without the feature; every consumer falls back to a no-op.
#[derive(Clone, Copy)]
pub struct NativeAccounts {
    /// Loads account configs from disk and returns boxed trait impls.
    pub accounts_init: fn() -> anyhow::Result<Vec<Box<dyn crate::account::Account>>>,
    /// Wires onboarding callbacks onto the `AppWindow`.
    pub onboarding_init: fn(&AppWindow),
    /// Refocus onboarding keyboard input (Windows webview focus dance).
    pub onboarding_focus_input: fn(),
}

static NATIVE_ACCOUNTS: OnceLock<Option<NativeAccounts>> = OnceLock::new();

pub fn set_native_accounts(hooks: Option<NativeAccounts>) {
    let _ = NATIVE_ACCOUNTS.set(hooks);
}

pub fn native_accounts() -> Option<NativeAccounts> {
    *NATIVE_ACCOUNTS.get().unwrap_or(&None)
}

fn shutdown(ui: &AppWindow) -> Result<(), Box<dyn Error>> {
    crate::thread_manager::join_all_threads();
    state_ctl().save(ui);
    config::config().save()?;
    Ok(())
}

/// Application entry point. Runs the whole UI startup/shutdown sequence.
pub fn start(native: Option<NativeAccounts>) -> Result<(), Box<dyn std::error::Error>> {
    set_native_accounts(native);
    let _logger = logging::init(&config::default_data_dir())?;

    crate::platform::init()?;
    let ui = AppWindow::new()?;
    crate::platform::setup_window(&ui);

    *WEAK_UI.lock().expect("ui lock is poisoned") = Some(ui.as_weak());

    let mut sidebar = Sidebar::new(&ui);
    ui.window().on_winit_window_event(move |_window, event| {
        crate::platform::on_window_event(_window, event);
        if sidebar.handle_event(event) == EventResult::PreventDefault {
            return EventResult::PreventDefault;
        }
        if let winit::event::WindowEvent::Focused(focus) = event
            && *focus
        {
            crate::platform::on_focus_gained();
            state_ctl().on_focus();
        }
        EventResult::Propagate
    });

    crate::database::init()?;
    crate::plugin::init()?;
    state_ctl().init();

    state_ctl().init_ui(&ui);
    state_ctl().sync_ui(&ui);
    if let Some(native) = native_accounts() {
        (native.onboarding_init)(&ui);
    }

    crate::updater::spawn_check_update_timer();

    let _event_pump = crate::platform::start_event_pump();

    match ui.run() {
        Err(e) => log::warn!("Error in UI thread occured: {e}"),
        _ => log::info!("UI exited successfully"),
    }
    shutdown(&ui)?;

    log::info!("Shutdown successful");

    Ok(())
}
