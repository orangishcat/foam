use std::io;

use crate::AppWindow;

#[derive(Clone, Default)]
pub struct SearchState {}

impl SearchState {
    pub fn sync_ui(&self, ui: &AppWindow) -> io::Result<()> {
        Ok(())
    }
}
