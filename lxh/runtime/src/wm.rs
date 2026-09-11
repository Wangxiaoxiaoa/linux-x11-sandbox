use lxh_core::LxhError;

use crate::process::ManagedProcess;

pub struct OpenboxWM;

impl OpenboxWM {
    pub async fn start(display: &str) -> Result<ManagedProcess, LxhError> {
        ManagedProcess::spawn("openbox", &["--replace"], &[("DISPLAY", display)]).await
    }
}
