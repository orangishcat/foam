use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const APP_NAME: &str = concat!("dev.orangishcat.", env!("CARGO_PKG_NAME"));
const CONFIG_FILE_NAME: &str = "config.json";

static CONFIG: LazyLock<RwLock<AppConfig>> = LazyLock::new(|| RwLock::new(AppConfig::load(None)));

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct AppConfig {
    pub subdomain: String,
    pub last_updated: Option<DateTime<Utc>>,
    pub user_id: String,
    pub cookie_key: String,
    pub cookie_value: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    #[serde(skip)]
    data_dir: PathBuf,
}

impl AppConfig {
    pub fn load(data_path: Option<PathBuf>) -> Self {
        let data_dir = data_path.unwrap_or_else(|| {
            dirs::data_dir()
                .expect("failed to locate the platform application data directory")
                .join(APP_NAME)
        });
        Self::create_data_layout(&data_dir);

        let config_file = data_dir.join(CONFIG_FILE_NAME);
        Self::create_default_file(&config_file);

        let contents = fs::read_to_string(&config_file).unwrap_or_else(|error| {
            panic!(
                "failed to read app configuration at {}: {error}",
                config_file.display()
            )
        });
        let mut config: Self = serde_json::from_str(&contents).unwrap_or_else(|error| {
            panic!(
                "failed to parse app configuration at {}: {error}",
                config_file.display()
            )
        });
        config.data_dir = data_dir;
        config
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn courses_dir(&self) -> PathBuf {
        self.data_dir().join("courses")
    }

    pub fn save(&self) -> std::io::Result<()> {
        let contents =
            serde_json::to_string_pretty(self).expect("failed to serialize app configuration");
        fs::write(
            self.data_dir.join(CONFIG_FILE_NAME),
            format!("{contents}\n"),
        )
    }

    fn create_data_layout(data_dir: &Path) {
        fs::create_dir_all(data_dir.join("courses")).expect("failed to create app data directory");
    }

    fn create_default_file(config_file: &Path) {
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(config_file)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => return,
            Err(error) => panic!(
                "failed to create default app configuration at {}: {error}",
                config_file.display()
            ),
        };

        let contents = serde_json::to_string_pretty(&Self::default())
            .expect("failed to serialize default app configuration");
        file.write_all(format!("{contents}\n").as_bytes())
            .expect("failed to write default app configuration");
    }
}

pub fn config() -> RwLockReadGuard<'static, AppConfig> {
    CONFIG.read().expect("config lock is poisoned")
}

pub fn config_write() -> RwLockWriteGuard<'static, AppConfig> {
    CONFIG.write().expect("config lock is poisoned")
}
