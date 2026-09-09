use std::sync::atomic::{AtomicU32, Ordering};

use lxs_core::LxsError;

pub mod display;
pub mod process;
pub mod wm;
pub mod xserver;

pub use display::{Display, DisplayConfig};

pub struct Runtime {
    next_display: AtomicU32,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            next_display: AtomicU32::new(99),
        }
    }

    pub async fn create_display(&self, config: DisplayConfig) -> Result<Display, LxsError> {
        let num = self.next_display.fetch_add(1, Ordering::SeqCst);
        let id = format!("d-{}", num);
        let display = format!(":{}", num);
        Display::create(id, display, config).await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
