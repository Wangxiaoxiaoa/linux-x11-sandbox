use lxs_core::LxsError;

use crate::process::ManagedProcess;

pub struct XvfbBackend;

impl XvfbBackend {
    pub async fn start(
        display: &str,
        width: u32,
        height: u32,
        depth: u32,
    ) -> Result<ManagedProcess, LxsError> {
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
    pub async fn start(display: &str, width: u32, height: u32) -> Result<ManagedProcess, LxsError> {
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
