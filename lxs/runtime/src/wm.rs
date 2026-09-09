use lxs_core::LxsError;

use crate::process::ManagedProcess;

pub struct OpenboxWM;

impl OpenboxWM {
    pub async fn start(display: &str) -> Result<ManagedProcess, LxsError> {
        ManagedProcess::spawn_with_env("openbox", &["--replace"], &[("DISPLAY", display)]).await
    }
}
