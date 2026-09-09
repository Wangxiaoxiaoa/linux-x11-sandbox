use lxs_core::LxsError;

use crate::process::ManagedProcess;

pub struct OpenboxWM;

impl OpenboxWM {
    pub async fn start(display: &str) -> Result<ManagedProcess, LxsError> {
        ManagedProcess::spawn("openbox", &["--replace"], &[("DISPLAY", display)]).await
    }
}
