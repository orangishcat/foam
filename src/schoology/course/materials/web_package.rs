use crate::types::LooseString;

use super::types::LooseInt;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebPackage {
    pub id: LooseString,
    pub title: String,
    pub uid: LooseInt,
    pub url: String,
}
