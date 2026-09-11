use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::time::timeout;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn bin_path() -> PathBuf {
    env::var("CARGO_BIN_EXE_linux-x11-harness")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/debug/linux-x11-harness"))
}

struct DaemonGuard {
    socket: PathBuf,
    pid: PathBuf,
    child: tokio::process::Child,
}

struct Connection {
    writer: tokio::net::unix::OwnedWriteHalf,
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    next_id: i64,
}

impl Connection {
    async fn call_method(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": method,
            "params": params
        });
        self.send(&req).await
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send(&req).await
    }

    async fn send(&mut self, req: &Value) -> Value {
        let line = req.to_string() + "\n";
        self.writer.write_all(line.as_bytes()).await.expect("write");
        self.writer.flush().await.expect("flush");

        let mut response = String::new();
        timeout(
            Duration::from_secs(10),
            self.reader.read_line(&mut response),
        )
        .await
        .expect("timeout")
        .expect("read");
        serde_json::from_str(&response).expect("parse response")
    }
}

impl DaemonGuard {
    async fn new() -> Self {
        let tmp = env::temp_dir();
        let unique = format!(
            "{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let socket = tmp.join(format!("lxh-test-{unique}.sock"));
        let pid = tmp.join(format!("lxh-test-{unique}.pid"));
        let _ = tokio::fs::remove_file(&socket).await;
        let _ = tokio::fs::remove_file(&pid).await;

        let mut child = Command::new(bin_path())
            .arg("serve")
            .env("LXH_SOCKET_PATH", &socket)
            .env("LXH_PID_PATH", &pid)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn daemon");

        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if socket.exists() {
                break;
            }
        }
        if !socket.exists() {
            let _ = child.kill().await;
            panic!("daemon failed to start");
        }

        Self { socket, pid, child }
    }

    async fn connect(&self) -> Connection {
        let stream = UnixStream::connect(&self.socket).await.expect("connect");
        let (reader, writer) = stream.into_split();
        Connection {
            writer,
            reader: BufReader::new(reader),
            next_id: 0,
        }
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let pid = self.child.id().unwrap_or(0);
        if pid > 0 {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        let _ = std::fs::remove_file(&self.socket);
        let _ = std::fs::remove_file(&self.pid);
    }
}

#[tokio::test]
async fn tools_list_returns_all_tools() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let resp = conn.call_method("tools/list", json!({})).await;
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 26, "expected 26 tools");

    for tool in tools {
        assert!(tool["name"].is_string(), "tool missing name: {tool}");
        assert!(
            tool["description"].is_string(),
            "tool missing description: {tool}"
        );
        assert!(
            tool["inputSchema"].is_object(),
            "tool missing inputSchema: {tool}"
        );
    }
}

#[tokio::test]
async fn create_and_destroy_harness_display() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"]
        .as_str()
        .expect("display_id")
        .to_string();
    let display = create["result"]["display"]
        .as_str()
        .expect("display")
        .to_string();
    assert!(
        display_id.starts_with("d-") && display_id.len() == 34,
        "display_id should be d-<uuid>: {display_id}"
    );
    assert!(display.starts_with(":"));

    let info = conn
        .call_tool("lxh_display_info", json!({"display_id": display_id}))
        .await;
    assert!(
        info["result"]["display"].is_string(),
        "unexpected info response: {}",
        info
    );

    let destroy = conn
        .call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
    assert!(destroy["result"]["success"].as_bool().unwrap());
}

#[tokio::test]
async fn launch_app_and_take_screenshot() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();
    let display = create["result"]["display"].as_str().unwrap().to_string();
    assert!(
        display_id.starts_with("d-") && display_id.len() == 34,
        "display_id should be d-<uuid>: {display_id}"
    );
    assert!(display.starts_with(":"));

    let launch = conn
        .call_tool(
            "lxh_app_launch",
            json!({"display_id": display_id, "command": "xterm", "args": ["-e", "sleep", "60"]}),
        )
        .await;
    assert!(
        launch["result"]["pid"].as_u64().unwrap() > 0,
        "unexpected launch response: {}",
        launch
    );

    tokio::time::sleep(Duration::from_millis(500)).await;

    let shot = conn
        .call_tool("lxh_capture_screenshot", json!({"display_id": display_id}))
        .await;
    assert!(!shot["result"]["data"].as_str().unwrap().is_empty());

    let _ = conn
        .call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}
