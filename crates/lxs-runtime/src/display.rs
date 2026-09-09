use std::sync::Arc;
use std::time::Duration;

use lxs_core::{Driver, LxsError};
use lxs_driver::NativeDriver;

use crate::process::ManagedProcess;
use crate::wm::OpenboxWM;
use crate::xserver::XvfbBackend;

#[derive(Clone)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub backend: Backend,
}

#[derive(Clone, Copy)]
pub enum Backend {
    Xvfb,
    Xephyr,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
            depth: 24,
            backend: Backend::Xvfb,
        }
    }
}

pub struct Display {
    id: String,
    display: String,
    xserver: ManagedProcess,
    wm: ManagedProcess,
    driver: Arc<dyn Driver>,
}

impl Display {
    pub async fn create(id: String, display: String, config: DisplayConfig) -> Result<Self, LxsError> {
        let xserver = match config.backend {
            Backend::Xvfb => XvfbBackend::start(&display, config.width, config.height, config.depth).await?,
            Backend::Xephyr => crate::xserver::XephyrBackend::start(&display, config.width, config.height).await?,
        };

        tokio::time::sleep(Duration::from_millis(500)).await;

        let wm = OpenboxWM::start(&display).await?;
        let driver = Arc::new(NativeDriver::new(&display));

        Ok(Self {
            id,
            display,
            xserver,
            wm,
            driver,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn driver(&self) -> Arc<dyn Driver> {
        self.driver.clone()
    }

    pub async fn destroy(&mut self) -> Result<(), LxsError> {
        let _ = self.wm.kill().await;
        let _ = self.xserver.kill().await;
        Ok(())
    }
}
