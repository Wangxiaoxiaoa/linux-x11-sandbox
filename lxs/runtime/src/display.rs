use std::sync::Arc;

use lxs_core::{DisplayInfo, Driver, LxsError};
use lxs_driver::NativeDriver;

use crate::process::ManagedProcess;
use crate::wm::OpenboxWM;
use crate::xserver::{XephyrBackend, XvfbBackend};

#[derive(Clone, Default)]
pub enum Backend {
    #[default]
    Xvfb,
    Xephyr,
}

#[derive(Clone)]
pub struct DisplayConfig {
    pub backend: Backend,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            backend: Backend::default(),
            width: 1280,
            height: 800,
            depth: 24,
        }
    }
}

pub struct Display {
    id: String,
    display: String,
    width: u32,
    height: u32,
    xserver: ManagedProcess,
    wm: ManagedProcess,
    apps: std::sync::Mutex<Vec<ManagedProcess>>,
    driver: Arc<dyn Driver>,
}

impl Display {
    pub async fn create(
        id: String,
        display: String,
        config: DisplayConfig,
    ) -> Result<Self, LxsError> {
        let xserver = match config.backend {
            Backend::Xvfb => {
                XvfbBackend::start(&display, config.width, config.height, config.depth).await?
            }
            Backend::Xephyr => XephyrBackend::start(&display, config.width, config.height).await?,
        };

        let wm = OpenboxWM::start(&display).await?;
        let driver = Arc::new(NativeDriver::new(&display)?);

        Ok(Self {
            id,
            display: display.clone(),
            width: config.width,
            height: config.height,
            xserver,
            wm,
            apps: std::sync::Mutex::new(Vec::new()),
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

    pub async fn launch_app(&self, command: &str, args: &[&str]) -> Result<u32, LxsError> {
        let proc = ManagedProcess::spawn(command, args, &[("DISPLAY", &self.display)]).await?;
        let pid = proc.pid();
        self.apps.lock().unwrap().push(proc);
        Ok(pid)
    }

    pub async fn terminate_app(&self, pid: u32) -> Result<(), LxsError> {
        let mut proc = {
            let mut apps = self.apps.lock().unwrap();
            let pos = apps
                .iter()
                .position(|p| p.pid() == pid)
                .ok_or_else(|| LxsError::DisplayNotFound(format!("pid {}", pid)))?;
            apps.remove(pos)
        };
        proc.kill().await
    }

    pub fn info(&self) -> DisplayInfo {
        DisplayInfo {
            display: self.display.clone(),
            width: self.width,
            height: self.height,
            app_count: self.apps.lock().unwrap().len(),
        }
    }

    pub async fn destroy(&mut self) -> Result<(), LxsError> {
        let mut apps = {
            let mut apps = self.apps.lock().unwrap();
            std::mem::take(&mut *apps)
        };
        for app in apps.iter_mut() {
            let _ = app.kill().await;
        }
        let _ = self.wm.kill().await;
        let _ = self.xserver.kill().await;
        Ok(())
    }
}
