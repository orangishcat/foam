use std::{cell::RefCell, io, time::Duration};

use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use wry::{
    Rect, WebView, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
};

use crate::{
    AppWindow, OnboardingUi, Screen, UiState, account::Account,
    api::schoology::account::SchoologyAccountConfig,
};

#[derive(Default)]
struct Onboarding {
    webview: Option<WebView>,
    account: SchoologyAccountConfig,
    url: String,
}

thread_local! { static FLOW: RefCell<Onboarding> = RefCell::default(); }

pub fn init(ui: &AppWindow) {
    ui.on_edit_webview(|action| {
        #[cfg(target_os = "macos")]
        crate::platform::macos::edit_webview(action);
        #[cfg(not(target_os = "macos"))]
        let _ = action;
    });
    if crate::account::active_account().is_err() {
        ui.global::<UiState>().set_screen(Screen::Onboarding);
    }
    let weak = ui.as_weak();
    ui.on_update_onboarding_webview(move || {
        if let Some(ui) = weak.upgrade() {
            update(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.global::<OnboardingUi>().on_previous(move || {
        if let Some(ui) = weak.upgrade() {
            let flow = ui.global::<OnboardingUi>();
            if flow.get_busy() {
                return;
            }
            flow.set_step((flow.get_step() - 1).max(0));
            flow.set_error("".into());
            update(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.global::<OnboardingUi>().on_next(move || {
        if let Some(ui) = weak.upgrade() {
            if let Err(error) = next(&ui) {
                ui.global::<OnboardingUi>()
                    .set_error(error.to_string().into());
            }
        }
    });
}

fn update(ui: &AppWindow) {
    let step = ui.global::<OnboardingUi>().get_step();
    let visible =
        ui.global::<UiState>().get_screen() == Screen::Onboarding && (2..=4).contains(&step);
    let bounds = Rect {
        position: LogicalPosition::new(ui.get_onboarding_x() as f64, ui.get_onboarding_y() as f64)
            .into(),
        size: LogicalSize::new(
            ui.get_onboarding_width().max(0.0) as f64,
            ui.get_onboarding_height().max(0.0) as f64,
        )
        .into(),
    };
    let result = FLOW.with(|flow| -> wry::Result<()> {
        let mut flow = flow.borrow_mut();
        if visible {
            let url = match step {
                2 => format!("https://{}.schoology.com", flow.account.subdomain),
                3 => format!("https://{}.schoology.com/api", flow.account.subdomain),
                4 => format!("https://{}.schoology.com/calendar", flow.account.subdomain),
                _ => "about:blank".into(),
            };
            if flow.webview.is_none() {
                ui.window()
                    .with_winit_window(|window| {
                        flow.webview = Some(
                            WebViewBuilder::new()
                                .with_bounds(bounds)
                                .with_url(&url)
                                .build_as_child(window)?,
                        );
                        Ok::<_, wry::Error>(())
                    })
                    .transpose()?;
            } else if flow.url != url {
                flow.webview.as_ref().unwrap().load_url(&url)?;
            }
            flow.url = url;
        }
        if let Some(webview) = &flow.webview {
            webview.set_visible(visible)?;
            webview.set_bounds(bounds)?;
        }
        Ok(())
    });
    if let Err(error) = result {
        ui.global::<OnboardingUi>()
            .set_error(format!("Could not open Schoology: {error}").into());
    }
}

fn next(ui: &AppWindow) -> crate::account::RequestResult<()> {
    let fields = ui.global::<OnboardingUi>();
    if fields.get_busy() {
        return Ok(());
    }
    fields.set_error("".into());
    match fields.get_step() {
        0 => fields.set_step(1),
        1 => {
            let subdomain = fields.get_subdomain().trim().to_ascii_lowercase();
            if !valid_subdomain(&subdomain) {
                return Err(io::Error::other(
                    "Enter a subdomain using letters, numbers, and hyphens.",
                )
                .into());
            }
            FLOW.with(|flow| {
                let mut flow = flow.borrow_mut();
                if flow.account.subdomain != subdomain {
                    flow.account = SchoologyAccountConfig {
                        subdomain: subdomain.clone(),
                        ..Default::default()
                    };
                }
            });
            fields.set_subdomain(subdomain.into());
            fields.set_step(2);
        }
        2 => {
            let account = FLOW.with(|flow| -> crate::account::RequestResult<_> {
                let flow = flow.borrow();
                let url = format!("https://{}.schoology.com/", flow.account.subdomain);
                let cookies = flow
                    .webview
                    .as_ref()
                    .ok_or_else(|| io::Error::other("The login panel is not ready."))?
                    .cookies_for_url(&url)?;
                let cookie = cookies
                    .iter()
                    .find(|cookie| {
                        (cookie.name().starts_with("SESS") || cookie.name().starts_with("SSESS"))
                            && !cookie.value().is_empty()
                    })
                    .ok_or_else(|| io::Error::other("Sign in to Schoology before continuing."))?;
                Ok(SchoologyAccountConfig {
                    subdomain: flow.account.subdomain.clone(),
                    cookie_key: cookie.name().into(),
                    cookie_value: cookie.value().into(),
                    ..Default::default()
                })
            })?;
            return worker(
                ui,
                move || {
                    // Validate without the existing cookie jar, which persists refreshed cookies.
                    let response: serde_json::Value = reqwest::blocking::Client::builder()
                        .timeout(Duration::from_secs(30))
                        .redirect(reqwest::redirect::Policy::none())
                        .build()?
                        .get(format!(
                            "https://{}.schoology.com/iapi2/site-navigation/notifications",
                            account.subdomain
                        ))
                        .header(
                            reqwest::header::COOKIE,
                            format!("{}={}", account.cookie_key, account.cookie_value),
                        )
                        .header(reqwest::header::ACCEPT, "application/json")
                        .send()?
                        .error_for_status()?
                        .json()?;
                    if !response.get("data").is_some_and(|data| data.is_array()) {
                        return Err(io::Error::other("Schoology did not return authenticated notifications. Please sign in and try again.").into());
                    }
                    Ok(account)
                },
                3,
            );
        }
        3 => {
            let key = fields.get_api_key().trim().to_owned();
            let secret = fields.get_api_secret().trim().to_owned();
            if key.is_empty() || secret.is_empty() {
                return Err(io::Error::other("Paste both the API key and secret.").into());
            }
            let mut account = FLOW.with(|flow| flow.borrow().account.clone());
            account.api_key = Some(key);
            account.api_secret = Some(secret);
            account.api_client = Default::default();
            return verify_api_user(ui, account);
        }
        4 => {
            let mut account = FLOW.with(|flow| flow.borrow().account.clone());
            let url = fields.get_calendar_url().trim().to_owned();
            if !valid_calendar(&url, &account.subdomain) {
                return Err(io::Error::other(
                    "Paste a webcal:// link from this school's /calendar/feed/ical/ export.",
                )
                .into());
            }
            account.calendar_url = url;
            std::fs::create_dir_all(crate::config::config().data_dir().join("plugins"))?;
            account.save()?;
            crate::account::set_accounts(vec![Box::new(account)]);
            fields.set_api_key("".into());
            fields.set_api_secret("".into());
            ui.global::<UiState>().set_screen(Screen::Dashboard);
            FLOW.with(|flow| *flow.borrow_mut() = Onboarding::default());
            crate::state::state::state().notif.last_update = chrono::DateTime::default();
            crate::state::state::state().on_focus();
            return Ok(());
        }
        _ => return Ok(()),
    }
    update(ui);
    Ok(())
}

fn verify_api_user(
    ui: &AppWindow,
    account: SchoologyAccountConfig,
) -> crate::account::RequestResult<()> {
    let weak = ui.as_weak();
    ui.global::<OnboardingUi>().set_busy(true);
    let result = FLOW.with(|flow| -> crate::account::RequestResult<()> {
        let flow = flow.borrow();
        let webview = flow.webview.as_ref()
            .ok_or_else(|| io::Error::other("The Schoology panel is not ready."))?;
        webview.evaluate_script_with_callback(
            r#"(() => {
                const button = document.querySelector('#header > header > nav > ul._2trRU._2K08O.fSqCh._1tpub.Header-ul-right-4WrfR > li:nth-child(3) > div > button');
                if (!button) return false;
                if (button.getAttribute('aria-expanded') !== 'true') button.click();
                return true;
            })()"#,
            move |value| {
                let account = account.clone();
                let opened = serde_json::from_str::<bool>(&value).unwrap_or(false);
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    if !opened {
                        ui.global::<OnboardingUi>().set_busy(false);
                        ui.global::<OnboardingUi>().set_error(
                            "Could not open your Schoology profile menu. Wait for Schoology to load and try again.".into(),
                        );
                        return;
                    }
                    let weak = ui.as_weak();
                    slint::Timer::single_shot(Duration::from_millis(250), move || {
                        if let Some(ui) = weak.upgrade() {
                            if let Err(error) = read_api_user(&ui, account) {
                                ui.global::<OnboardingUi>().set_busy(false);
                                ui.global::<OnboardingUi>().set_error(error.to_string().into());
                            }
                        }
                    });
                });
            },
        )?;
        Ok(())
    });
    if result.is_err() {
        ui.global::<OnboardingUi>().set_busy(false);
    }
    result
}

fn read_api_user(
    ui: &AppWindow,
    account: SchoologyAccountConfig,
) -> crate::account::RequestResult<()> {
    let host = serde_json::to_string(&format!("{}.schoology.com", account.subdomain))?;
    let script = format!(
        r#"(() => {{
            for (const anchor of document.querySelectorAll('a[href]')) {{
                const url = new URL(anchor.href, location.href);
                const match = url.pathname.match(/^\/user\/(\d+)\/info\/?$/);
                if (url.protocol === 'https:' && url.hostname === {host} && match) {{
                    return match[1];
                }}
            }}
            return null;
        }})()"#
    );
    let weak = ui.as_weak();
    ui.global::<OnboardingUi>().set_busy(true);
    let result = FLOW.with(|flow| -> crate::account::RequestResult<()> {
        let flow = flow.borrow();
        let webview = flow.webview.as_ref()
            .ok_or_else(|| io::Error::other("The Schoology panel is not ready."))?;
        webview.evaluate_script_with_callback(&script, move |value| {
            let mut account = account.clone();
            let id = serde_json::from_str::<Option<String>>(&value).ok().flatten()
                .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let Some(id) = id else {
                    ui.global::<OnboardingUi>().set_busy(false);
                    ui.global::<OnboardingUi>().set_error(
                        "Could not find your /user/<id>/info profile link. Wait for Schoology to load and try again.".into(),
                    );
                    return;
                };
                account.user_id = id;
                if let Err(error) = worker(&ui, move || {
                    let _: serde_json::Value = account.api_get(&format!(
                        "https://api.schoology.com/v1/users/{}", account.user_id,
                    ))?;
                    Ok(account)
                }, 4) {
                    ui.global::<OnboardingUi>().set_error(error.to_string().into());
                }
            });
        })?;
        Ok(())
    });
    if result.is_err() {
        ui.global::<OnboardingUi>().set_busy(false);
    }
    result
}

fn worker(
    ui: &AppWindow,
    task: impl FnOnce() -> crate::account::RequestResult<SchoologyAccountConfig> + Send + 'static,
    step: i32,
) -> crate::account::RequestResult<()> {
    let weak = ui.as_weak();
    ui.global::<OnboardingUi>().set_busy(true);
    if let Err(error) = crate::thread_manager::spawn_thread("Schoology onboarding", move || {
        let result = task().inspect_err(|err|log::warn!("Could not verify Schoology account: {err}")).map_err(|_| "Could not verify your Schoology account. Check your login or API credentials and try again.");
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.global::<OnboardingUi>().set_busy(false);
            match result {
                Ok(account) => {
                    FLOW.with(|flow| flow.borrow_mut().account = account);
                    ui.global::<OnboardingUi>().set_step(step);
                    update(&ui);
                }
                Err(error) => ui.global::<OnboardingUi>().set_error(error.into()),
            }
        });
    }) {
        ui.global::<OnboardingUi>().set_busy(false);
        return Err(error.into());
    }
    Ok(())
}

fn valid_subdomain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_calendar(value: &str, subdomain: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "webcal"
            && url.host_str() == Some(format!("{subdomain}.schoology.com").as_str())
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url
                .path()
                .strip_prefix("/calendar/feed/ical/")
                .is_some_and(|feed| !feed.is_empty())
    })
}
