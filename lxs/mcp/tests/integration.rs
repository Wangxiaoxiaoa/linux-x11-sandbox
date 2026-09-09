use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

struct McpSession {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    writer: std::process::ChildStdin,
    next_id: i64,
}

impl McpSession {
    fn new() -> Self {
        let bin = env!("CARGO_BIN_EXE_linux-x11-sandbox");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn linux-x11-sandbox");

        let reader = BufReader::new(child.stdout.take().unwrap());
        let writer = child.stdin.take().unwrap();

        Self {
            child,
            reader,
            writer,
            next_id: 1,
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let msg = req.to_string() + "\n";
        self.writer.write_all(msg.as_bytes()).unwrap();
        self.writer.flush().unwrap();

        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).expect("invalid json response")
    }

    fn create_display(&mut self) -> String {
        let resp = self.call(
            "tools/call",
            json!({"name": "lxs_display_create", "arguments": {}}),
        );
        assert!(
            resp["result"]["display"].as_str().unwrap().starts_with(':'),
            "display create failed: {resp}"
        );
        resp["result"]["display_id"].as_str().unwrap().to_string()
    }

    fn destroy_display(&mut self, id: &str) {
        let resp = self.call(
            "tools/call",
            json!({
                "name": "lxs_display_destroy",
                "arguments": { "display_id": id },
            }),
        );
        assert_eq!(
            resp["result"]["success"], true,
            "display destroy failed: {resp}"
        );
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_smoke() {
    let mut s = McpSession::new();

    let init = s.call(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "smoke", "version": "0.1.0" },
        }),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "linux-x11-sandbox");

    let tools = s.call("tools/list", json!({}));
    let names: Vec<String> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();

    let required = [
        "lxs_display_create",
        "lxs_display_destroy",
        "lxs_display_info",
        "lxs_app_launch",
        "lxs_app_terminate",
        "lxs_app_list",
        "lxs_input_click",
        "lxs_input_move",
        "lxs_input_scroll",
        "lxs_input_type",
        "lxs_input_key",
        "lxs_capture_screenshot",
        "lxs_capture_region",
        "lxs_state_window",
        "lxs_state_tree",
        "lxs_state_element_bounds",
        "lxs_perform_action",
    ];
    for name in required {
        assert!(names.contains(&name.to_string()), "missing tool {}", name);
    }

    let id = s.create_display();
    s.destroy_display(&id);
}

#[test]
fn app_lifecycle() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let launch = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": { "display_id": id, "command": "xterm", "args": [] },
        }),
    );
    let pid = launch["result"]["pid"].as_u64().unwrap();
    assert!(pid > 0);

    thread::sleep(Duration::from_millis(500));

    let list = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_list",
            "arguments": { "display_id": id },
        }),
    );
    let pids: Vec<u64> = list["result"]["pids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(pids.contains(&pid));

    let terminate = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_terminate",
            "arguments": { "display_id": id, "pid": pid },
        }),
    );
    assert_eq!(terminate["result"]["success"], true);

    let list = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_list",
            "arguments": { "display_id": id },
        }),
    );
    let pids: Vec<u64> = list["result"]["pids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert!(!pids.contains(&pid));

    s.destroy_display(&id);
}

#[test]
fn screenshot_is_valid_png() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let info = s.call(
        "tools/call",
        json!({
            "name": "lxs_display_info",
            "arguments": { "display_id": id },
        }),
    );
    let width = info["result"]["width"].as_u64().unwrap() as u32;
    let height = info["result"]["height"].as_u64().unwrap() as u32;

    let shot = s.call(
        "tools/call",
        json!({
            "name": "lxs_capture_screenshot",
            "arguments": { "display_id": id },
        }),
    );
    assert_eq!(shot["result"]["mimeType"], "image/png");

    let data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        shot["result"]["data"].as_str().unwrap(),
    )
    .unwrap();
    assert!(data.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));

    let img = image::load_from_memory_with_format(&data, image::ImageFormat::Png).unwrap();
    assert_eq!(img.width(), width);
    assert_eq!(img.height(), height);

    s.destroy_display(&id);
}

#[test]
fn input_operations_do_not_error() {
    let mut s = McpSession::new();
    let id = s.create_display();

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_move",
            "arguments": { "display_id": id, "x": 100, "y": 100 },
        }),
    );

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_click",
            "arguments": { "display_id": id, "x": 100, "y": 100, "button": "left" },
        }),
    );

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_scroll",
            "arguments": { "display_id": id, "dx": 0, "dy": 3 },
        }),
    );

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_type",
            "arguments": { "display_id": id, "text": "hello" },
        }),
    );

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_key",
            "arguments": { "display_id": id, "key": "Return" },
        }),
    );

    s.destroy_display(&id);
}

#[test]
fn display_info_matches_config() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let info = s.call(
        "tools/call",
        json!({
            "name": "lxs_display_info",
            "arguments": { "display_id": id },
        }),
    );
    assert_eq!(info["result"]["width"], 1280);
    assert_eq!(info["result"]["height"], 800);
    assert_eq!(info["result"]["app_count"], 0);

    s.destroy_display(&id);
}

#[test]
fn region_screenshot_is_valid_png() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let shot = s.call(
        "tools/call",
        json!({
            "name": "lxs_capture_region",
            "arguments": { "display_id": id, "x": 10, "y": 20, "w": 100, "h": 80 },
        }),
    );
    assert_eq!(shot["result"]["mimeType"], "image/png");

    let data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        shot["result"]["data"].as_str().unwrap(),
    )
    .unwrap();
    assert!(data.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));

    let img = image::load_from_memory_with_format(&data, image::ImageFormat::Png).unwrap();
    assert_eq!(img.width(), 100);
    assert_eq!(img.height(), 80);

    s.destroy_display(&id);
}

#[test]
fn window_state_returns_title_after_launch() {
    let mut s = McpSession::new();
    let id = s.create_display();

    s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": { "display_id": id, "command": "xterm", "args": [] },
        }),
    );
    thread::sleep(Duration::from_millis(800));

    let state = s.call(
        "tools/call",
        json!({
            "name": "lxs_state_window",
            "arguments": { "display_id": id },
        }),
    );
    let title = state["result"]["title"].as_str();
    assert!(
        title.map(|t| !t.is_empty()).unwrap_or(false),
        "expected non-empty window title, got {:?}",
        title
    );

    s.destroy_display(&id);
}

#[test]
fn invalid_display_id_returns_error() {
    let mut s = McpSession::new();
    let resp = s.call(
        "tools/call",
        json!({
            "name": "lxs_display_info",
            "arguments": { "display_id": "does-not-exist" },
        }),
    );
    assert!(
        resp["error"].is_object(),
        "expected error response, got {resp}"
    );
}

#[test]
fn terminate_missing_pid_returns_error() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let resp = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_terminate",
            "arguments": { "display_id": id, "pid": 99999999 },
        }),
    );
    assert!(
        resp["error"].is_object(),
        "expected error response, got {resp}"
    );

    s.destroy_display(&id);
}

#[test]
#[ignore = "requires chromium to be installed"]
fn atspi_tree_is_populated_for_chromium() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let launch = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": {
                "display_id": id,
                "command": "chromium",
                "args": ["--no-sandbox", "--force-renderer-accessibility"],
            },
        }),
    );
    let pid = launch["result"]["pid"].as_u64().unwrap() as u32;
    thread::sleep(Duration::from_secs(3));

    let tree = s.call(
        "tools/call",
        json!({
            "name": "lxs_state_tree",
            "arguments": { "display_id": id, "pid": pid },
        }),
    );
    let elements = tree["result"]["elements"].as_array().unwrap();
    assert!(!elements.is_empty(), "AT-SPI tree was empty");

    let first = &elements[0];
    let index = first["index"].as_u64().unwrap() as usize;
    let bounds = s.call(
        "tools/call",
        json!({
            "name": "lxs_state_element_bounds",
            "arguments": { "display_id": id, "pid": pid, "index": index },
        }),
    );
    assert!(bounds["result"]["w"].as_u64().unwrap() > 0);
    assert!(bounds["result"]["h"].as_u64().unwrap() > 0);

    let _ = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_terminate",
            "arguments": { "display_id": id, "pid": pid },
        }),
    );
    s.destroy_display(&id);
}
