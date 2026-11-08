// todo: implement plugin system using wasm; this will have to do for now

use crate::{account::Account, api::schoology::account::SchoologyAccountConfig};
use anyhow;

pub fn init() -> anyhow::Result<Vec<Box<dyn Account>>> {
    Ok(match SchoologyAccountConfig::load() {
        Ok(acc) => vec![Box::new(acc)],
        Err(err) => {
            log::warn!("Failed to read Schoology config: {err}");
            vec![]
        }
    })
}
