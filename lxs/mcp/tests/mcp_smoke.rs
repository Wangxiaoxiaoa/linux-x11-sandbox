use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

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

    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_create", "arguments": {}}),
    );
    let display_id = display["result"]["display_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(display["result"]["display"]
        .as_str()
        .unwrap()
        .starts_with(':'));

    let info = s.call(
        "tools/call",
        json!({
            "name": "lxs_display_info",
            "arguments": { "display_id": display_id },
        }),
    );
    assert!(info["result"]["width"].as_u64().unwrap() > 0);
    assert!(info["result"]["height"].as_u64().unwrap() > 0);

    let shot = s.call(
        "tools/call",
        json!({
            "name": "lxs_capture_screenshot",
            "arguments": { "display_id": display_id },
        }),
    );
    assert_eq!(shot["result"]["mimeType"], "image/png");
    assert!(!shot["result"]["data"].as_str().unwrap().is_empty());

    let destroyed = s.call(
        "tools/call",
        json!({
            "name": "lxs_display_destroy",
            "arguments": { "display_id": display_id },
        }),
    );
    assert_eq!(destroyed["result"]["success"], true);
}
