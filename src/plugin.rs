// imaginary plugin api before i turn it into an actual plugin api using wasm or something

use crate::{
    account::{self},
    api::schoology::{self},
};

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    account::set_accounts(schoology::main::init()?);
    Ok(())
}
