use std::path::PathBuf;

use clapfig::{Clapfig, Schema, SearchPath};
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Schema, Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub counter: i8,
    pub cookie: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
}

impl AppConfig {
    pub fn load(config_path: Option<PathBuf>) -> Self {
        let builder = Clapfig::schema_builder::<AppConfig>().file_name(CONFIG_FILE_NAME);
        let builder = match config_path {
            Some(path) => builder.search_paths(vec![SearchPath::Path(path)]),
            None => builder,
        };
        builder.load().expect("failed to load app configuration")
    }
}
