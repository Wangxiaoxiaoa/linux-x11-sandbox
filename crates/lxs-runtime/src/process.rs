use std::process::Stdio;
use tokio::process::{Child, Command};

use lxs_core::LxsError;

pub struct ManagedProcess {
    child: Child,
    pid: u32,
}

impl ManagedProcess {
    pub async fn spawn(cmd: &str, args: &[&str]) -> Result<Self, LxsError> {
        let child = Command::new(cmd)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

        let pid = child.id().unwrap_or(0);
        Ok(Self { child, pid })
    }

    pub async fn spawn_with_env(cmd: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<Self, LxsError> {
        let mut command = Command::new(cmd);
        command
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        for (k, v) in envs {
            command.env(k, v);
        }

        let child = command
            .spawn()
            .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?;

        let pid = child.id().unwrap_or(0);
        Ok(Self { child, pid })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub async fn kill(&mut self) -> Result<(), LxsError> {
        self.child
            .kill()
            .await
            .map_err(|e| LxsError::ProcessKillFailed(e.to_string()))?;
        Ok(())
    }
}
