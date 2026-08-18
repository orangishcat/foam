use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use clapfig::{Clapfig, Schema, SearchPath};
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "config.toml";
const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Schema, Serialize, Deserialize, Debug, Clone, Default)]
pub struct AppConfig {
    #[clapfig(default = "")]
    pub cookie_key: String,
    #[clapfig(default = "")]
    pub cookie_value: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
}

impl AppConfig {
    pub fn load(config_path: Option<PathBuf>) -> Self {
        let config_dir = config_path.unwrap_or_else(|| {
            dirs::config_dir()
                .expect("failed to locate the platform configuration directory")
                .join(APP_NAME)
        });
        Self::create_default_file(&config_dir);

        let builder = Clapfig::schema_builder::<AppConfig>()
            .app_name(APP_NAME)
            .file_name(CONFIG_FILE_NAME)
            .search_paths(vec![SearchPath::Path(config_dir)]);
        builder.load().expect("failed to load app configuration")
    }

    fn create_default_file(config_dir: &Path) {
        fs::create_dir_all(config_dir).expect("failed to create app configuration directory");

        let config_file = config_dir.join(CONFIG_FILE_NAME);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config_file)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => return,
            Err(error) => panic!(
                "failed to create default app configuration at {}: {error}",
                config_file.display()
            ),
        };

        let contents = toml::to_string_pretty(&Self::default())
            .expect("failed to serialize default app configuration");
        file.write_all(contents.as_bytes())
            .expect("failed to write default app configuration");
    }
}
