use std::{error, io, sync::Mutex};

use anyhow;

pub type RequestError = Box<dyn error::Error + Send + Sync>;
pub type RequestResult<T> = std::result::Result<T, RequestError>;

static ACCOUNTS: Mutex<Vec<Box<dyn Account>>> = Mutex::new(vec![]);

pub trait Account: Send {
    fn clone_account(&self) -> Box<dyn Account>;
    fn assignment_url(&self, id: &str) -> String;
    fn resource_headers(&self, url: &str) -> RequestResult<reqwest::header::HeaderMap>;
    fn load() -> anyhow::Result<Self>
    where
        Self: Sized;
    fn save(&self) -> anyhow::Result<()>;
    fn fast_sync(&self) -> anyhow::Result<()>;
    fn slow_sync(&self) -> anyhow::Result<()>;
    fn get_resource(&self, url: &str) -> RequestResult<reqwest::blocking::RequestBuilder>;
}

pub fn active_account() -> anyhow::Result<Box<dyn Account>> {
    let accounts = ACCOUNTS
        .lock()
        .map_err(|_| io::Error::other("accounts lock is poisoned"))?;
    accounts
        .first() // todo: based on focused course
        .map(|account| account.clone_account())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No active account").into())
}

pub fn set_accounts(acc: Vec<Box<dyn Account>>) {
    *ACCOUNTS.lock().expect("acc lock is poisoned") = acc;
}
