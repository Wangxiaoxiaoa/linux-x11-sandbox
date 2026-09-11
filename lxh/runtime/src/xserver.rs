use lxh_core::LxhError;

use crate::process::ManagedProcess;

pub struct XvfbBackend;

impl XvfbBackend {
    pub async fn start(
        display: &str,
        width: u32,
        height: u32,
        depth: u32,
    ) -> Result<ManagedProcess, LxhError> {
        let screen = format!("{}x{}x{}", width, height, depth);
        ManagedProcess::spawn(
            "Xvfb",
            &[display, "-screen", "0", &screen, "-ac", "-noreset"],
            &[("DISPLAY", display)],
        )
        .await
    }
}

pub struct XephyrBackend;

impl XephyrBackend {
    pub async fn start(display: &str, width: u32, height: u32) -> Result<ManagedProcess, LxhError> {
        ManagedProcess::spawn(
            "Xephyr",
            &[
                display,
                "-screen",
                &format!("{}x{}", width, height),
                "-ac",
                "-br",
                "-noreset",
            ],
            &[("DISPLAY", display)],
        )
        .await
    }
}
