// imaginary plugin api before i turn it into an actual plugin api using wasm or something

use std::{io, sync::Mutex};

use crate::{
    account::{self, Account},
    api::schoology::{self},
};

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    account::set_accounts(schoology::main::init()?);
    Ok(())
}
