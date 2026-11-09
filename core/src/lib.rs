//! Foam core library. The binary (`app/src/main.rs`) is a thin launcher that
//! optionally registers native account plugins (e.g. `plugins/schoology`)
//! through [`NativeAccounts`]. Core modules are generic and provider-free:
//! only the accounts registry and startup seams know about plugins.

slint::include_modules!();

pub mod account;
pub mod config;
pub mod database;
pub mod embedded_webview;
pub mod filesystem;
pub mod logging;
pub mod platform;
pub mod plugin;
pub mod state;
pub mod theme;
pub mod thread_manager;
pub mod types;
pub mod ui;
pub mod updater;
pub mod wry;

pub use app::{NativeAccounts, native_accounts, set_native_accounts, start};

mod app;
