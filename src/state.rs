use crate::config::AppConfig;

pub struct State {
    config: AppConfig,
}

impl State {
    pub fn new(config: AppConfig) -> Self {
        State { config }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }
}
