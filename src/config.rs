use std::{
    fs::{self},
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use chrono::TimeDelta;
use derivative::Derivative;
use serde::{Deserialize, Serialize};

use crate::filesystem;

const APP_NAME: &str = concat!("dev.orangishcat.", env!("CARGO_PKG_NAME"));
const CONFIG_FILE_NAME: &str = "config.json";

static CONFIG: LazyLock<RwLock<AppConfig>> = LazyLock::new(|| RwLock::new(AppConfig::load(None)));

#[derive(Serialize, Deserialize, Debug, Clone, Derivative)]
#[derivative(Default)]
#[serde(default)]
pub struct AppConfig {
    pub subdomain: String,
    pub user_id: String,
    pub cookie_key: String,
    pub cookie_value: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,

    #[derivative(Default(value = "TimeDelta::minutes(5)"))]
    pub refresh_duration: TimeDelta,
    #[derivative(Default(value = "TimeDelta::minutes(60)"))]
    pub submission_refresh_duration: TimeDelta,

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

        let mut config: Self = filesystem::read_json(&data_dir.join(CONFIG_FILE_NAME))
            .inspect_err(|e| log::warn!("Failed to read config, using default: {e}"))
            .unwrap_or_default();
        config.data_dir = data_dir;
        config
    }

    pub fn save(&self) -> std::io::Result<()> {
        filesystem::write_json(&self.config_file(), self)
            .inspect_err(|e| log::warn!("Failed to write config: {e}"))
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn config_file(&self) -> PathBuf {
        self.data_dir.join(CONFIG_FILE_NAME)
    }

    pub fn courses_dir(&self) -> PathBuf {
        self.data_dir().join("courses")
    }

    fn create_data_layout(data_dir: &Path) {
        fs::create_dir_all(data_dir.join("courses")).expect("failed to create app data directory");
    }
}

pub fn config() -> RwLockReadGuard<'static, AppConfig> {
    CONFIG.read().expect("config lock is poisoned")
}

pub fn config_write() -> RwLockWriteGuard<'static, AppConfig> {
    CONFIG.write().expect("config lock is poisoned")
}
