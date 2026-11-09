// Launcher binary. All application logic lives in the `foam` core library;
// this file only wires the native builtin account plugins into the
// `NativeAccounts` startup seam. Foam builds without Schoology imports
// entirely when the `schoology-plugin` feature is off.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use foam::{NativeAccounts, start};

#[cfg(feature = "schoology-plugin")]
fn native_accounts() -> Option<NativeAccounts> {
    Some(NativeAccounts {
        accounts_init: schoology::accounts_init,
        onboarding_init: schoology::onboarding_init,
        onboarding_focus_input: schoology::focus_input,
    })
}

#[cfg(not(feature = "schoology-plugin"))]
fn native_accounts() -> Option<NativeAccounts> {
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    start(native_accounts())
}
