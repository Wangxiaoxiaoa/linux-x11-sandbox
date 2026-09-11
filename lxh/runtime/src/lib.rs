use std::sync::atomic::{AtomicU32, Ordering};

use lxh_core::LxhError;

pub mod display;
pub mod process;
pub mod wm;
pub mod xserver;

pub use display::{Backend, Display, DisplayConfig, DisplayKind};

pub struct Runtime {
    next_display: AtomicU32,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            next_display: AtomicU32::new(99),
        }
    }

    pub async fn create_display(&self, config: DisplayConfig) -> Result<Display, LxhError> {
        let num = self.next_display.fetch_add(1, Ordering::SeqCst);
        let id = format!("d-{}", num);
        let display = format!(":{}", num);
        Display::create(id, display, config).await
    }

    pub async fn attach_display(&self, display: &str) -> Result<Display, LxhError> {
        let id = display.to_string();
        Display::attach(id, display.to_string()).await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
