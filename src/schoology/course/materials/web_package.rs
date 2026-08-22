use crate::types::LooseString;

use super::types::LooseInt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebPackage {
    #[serde(default)]
    pub id: LooseString,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub uid: LooseInt,
    #[serde(default)]
    pub url: String,
}
