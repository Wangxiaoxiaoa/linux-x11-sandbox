use std::process::Stdio;
use tokio::process::{Child, Command};

use lxs_core::LxsError;

fn chromium_family_program(prog: &str) -> bool {
    let lower = prog.to_ascii_lowercase();
    lower.contains("chromium")
        || lower.contains("chrome")
        || lower.contains("brave")
        || lower.contains("edge")
        || lower.contains("opera")
}

fn ensure_accessibility_args(cmd: &str, args: &[&str]) -> Vec<String> {
    let mut result: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    if chromium_family_program(cmd) && !result.iter().any(|a| a == "--force-renderer-accessibility")
    {
        result.push("--force-renderer-accessibility".into());
    }
    result
}

pub struct ManagedProcess {
    child: Child,
    pid: u32,
}

impl ManagedProcess {
    pub async fn spawn(cmd: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<Self, LxsError> {
        let effective_args = ensure_accessibility_args(cmd, args);
        let mut command = Command::new(cmd);
        command
            .args(&effective_args)
            .env("ACCESSIBILITY_ENABLED", "1")
            .env("NO_AT_BRIDGE", "0")
            .env("QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "1")
            .env("QT_ACCESSIBILITY", "1")
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
