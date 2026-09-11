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

#[tokio::test]
async fn app_terminate_kills_process() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let launch = conn
        .call_tool(
            "lxh_app_launch",
            json!({"display_id": display_id, "command": "sleep", "args": ["60"]}),
        )
        .await;
    let pid = launch["result"]["pid"].as_u64().unwrap() as u32;
    assert!(pid > 0);

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(tokio::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .await
        .unwrap()
        .success());

    let term = conn
        .call_tool("lxh_app_terminate", json!({"display_id": display_id, "pid": pid}))
        .await;
    assert!(term["result"]["success"].as_bool().unwrap());

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!tokio::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .await
        .unwrap()
        .success());

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}

#[tokio::test]
async fn input_move_and_get_cursor_position() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let move_resp = conn
        .call_tool("lxh_input_move", json!({"display_id": display_id, "x": 123, "y": 456}))
        .await;
    assert!(move_resp["result"]["success"].as_bool().unwrap());

    tokio::time::sleep(Duration::from_millis(50)).await;

    let pos = conn
        .call_tool("lxh_input_get_cursor_position", json!({"display_id": display_id}))
        .await;
    assert_eq!(pos["result"]["x"].as_i64(), Some(123));
    assert_eq!(pos["result"]["y"].as_i64(), Some(456));

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}

#[tokio::test]
async fn clipboard_roundtrip() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let set = conn
        .call_tool(
            "lxh_clipboard_set",
            json!({"display_id": display_id, "text": "hello harness"}),
        )
        .await;
    assert!(set["result"]["success"].as_bool().unwrap());

    let get = conn
        .call_tool("lxh_clipboard_get", json!({"display_id": display_id}))
        .await;
    assert_eq!(get["result"]["text"].as_str(), Some("hello harness"));

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}

#[tokio::test]
async fn invalid_display_id_returns_error() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let resp = conn
        .call_tool("lxh_capture_screenshot", json!({"display_id": "d-doesnotexist"}))
        .await;
    assert!(resp["error"].is_object(), "expected error response: {resp}");
    assert!(resp["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[tokio::test]
async fn invalid_tool_arguments_returns_error() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let resp = conn
        .call_tool("lxh_app_launch", json!({"display_id": display_id}))
        .await;
    assert!(resp["error"].is_object(), "expected error response: {resp}");

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}

#[tokio::test]
async fn display_info_returns_resolution() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let info = conn
        .call_tool("lxh_display_info", json!({"display_id": display_id}))
        .await;
    assert!(info["result"]["width"].as_u64().unwrap() > 0);
    assert!(info["result"]["height"].as_u64().unwrap() > 0);

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}

#[tokio::test]
async fn xephyr_backend_when_available() {
    if tokio::process::Command::new("which")
        .arg("Xephyr")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        return;
    }

    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn
        .call_tool("lxh_display_create", json!({"backend": "xephyr"}))
        .await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let info = conn
        .call_tool("lxh_display_info", json!({"display_id": display_id}))
        .await;
    assert!(info["result"]["display"].is_string());

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}

#[tokio::test]
async fn desktop_overview_lists_launched_app() {
    let daemon = DaemonGuard::new().await;
    let mut conn = daemon.connect().await;
    let create = conn.call_tool("lxh_display_create", json!({})).await;
    let display_id = create["result"]["display_id"].as_str().unwrap().to_string();

    let launch = conn
        .call_tool(
            "lxh_app_launch",
            json!({"display_id": display_id, "command": "xterm", "args": ["-e", "sleep", "60"]}),
        )
        .await;
    let pid = launch["result"]["pid"].as_u64().unwrap();

    tokio::time::sleep(Duration::from_millis(800)).await;

    let overview = conn
        .call_tool("lxh_get_desktop_overview", json!({"display_id": display_id}))
        .await;
    let processes = overview["result"]["processes"].as_array().unwrap();
    assert!(
        processes.iter().any(|p| p["pid"].as_u64() == Some(pid)),
        "launched process not in overview: {overview}"
    );

    conn.call_tool("lxh_display_destroy", json!({"display_id": display_id}))
        .await;
}
