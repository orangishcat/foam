use std::{error, io, sync::Mutex};

pub type RequestError = Box<dyn error::Error + Send + Sync>;
pub type RequestResult<T> = Result<T, RequestError>;

static ACCOUNTS: Mutex<Vec<Box<dyn Account>>> = Mutex::new(vec![]);

pub trait Account: Send {
    fn clone_account(&self) -> Box<dyn Account>;
    fn assignment_url(&self, id: &str) -> String;
    fn resource_headers(&self, url: &str) -> RequestResult<reqwest::header::HeaderMap>;
    fn load() -> Result<Self, io::Error>
    where
        Self: Sized;
    fn save(&self) -> Result<(), io::Error>;
    fn fast_sync(&self) -> Result<(), io::Error>;
    fn slow_sync(&self) -> Result<(), io::Error>;
    fn get_resource(&self, url: &str) -> RequestResult<reqwest::blocking::RequestBuilder>;
}

pub fn active_account() -> io::Result<Box<dyn Account>> {
    let accounts = ACCOUNTS
        .lock()
        .map_err(|_| io::Error::other("accounts lock is poisoned"))?;
    accounts
        .first() // todo: based on focused course
        .map(|account| account.clone_account())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No active account"))
}

pub fn set_accounts(acc: Vec<Box<dyn Account>>) {
    *ACCOUNTS.lock().expect("acc lock is poisoned") = acc;
}
