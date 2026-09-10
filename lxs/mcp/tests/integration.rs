use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
        "lxs_input_click",
        "lxs_input_move",
        "lxs_input_scroll",
        "lxs_input_drag",
        "lxs_input_get_cursor_position",
        "lxs_input_type",
        "lxs_input_key",
        "lxs_window_focus",
        "lxs_window_set_frame",
        "lxs_window_close",
        "lxs_click_element",
        "lxs_set_value",
        "lxs_capture_screenshot",
        "lxs_capture_window",
        "lxs_clipboard_get",
        "lxs_clipboard_set",
        "lxs_get_desktop_overview",
        "lxs_get_window_state",
        "lxs_wait",
    ];
    for name in required {
        assert!(names.contains(&name.to_string()), "missing tool {}", name);
    }

    let id = s.create_display();
    s.destroy_display(&id);
}

#[test]
fn display_info_matches_config() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let info = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    );
    assert_eq!(info["result"]["width"], 1280);
    assert_eq!(info["result"]["height"], 800);
    assert!(info["result"]["display"].as_str().unwrap().starts_with(':'));

    s.destroy_display(&id);
}

#[test]
fn invalid_display_id_returns_error() {
    let mut s = McpSession::new();

    let resp = s.call(
        "tools/call",
        json!({
            "name": "lxs_display_info",
            "arguments": { "display_id": ":99999" },
        }),
    );
    assert!(
        resp["error"].is_object(),
        "expected error for invalid display id"
    );
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

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let pids: Vec<u64> = overview["result"]["processes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["pid"].as_u64().unwrap())
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

    thread::sleep(Duration::from_millis(300));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let pids: Vec<u64> = overview["result"]["processes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["pid"].as_u64().unwrap())
        .collect();
    assert!(!pids.contains(&pid));

    s.destroy_display(&id);
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
    assert!(resp["error"].is_object(), "expected error for missing pid");

    s.destroy_display(&id);
}

#[test]
fn pointer_click_reaches_xev() {
    let mut s = McpSession::new();
    let id = s.create_display();
    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    )["result"]["display"]
        .as_str()
        .unwrap()
        .to_string();

    let mut xev = Command::new("xev")
        .arg("-display")
        .arg(&display)
        .args(["-geometry", "200x200+50+50"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn xev");

    thread::sleep(Duration::from_millis(1000));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_click",
            "arguments": { "display_id": id, "x": 150, "y": 150 },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    let _ = xev.kill();
    let output = xev.wait_with_output().expect("xev wait failed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("ButtonPress"),
        "no button press detected: {text}"
    );
    assert!(
        text.contains("ButtonRelease"),
        "no button release detected: {text}"
    );
    assert!(
        text.contains("button 1"),
        "expected left button (1): {text}"
    );

    s.destroy_display(&id);
}

#[test]
fn click_right_and_double_reach_xev() {
    let mut s = McpSession::new();
    let id = s.create_display();
    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    )["result"]["display"]
        .as_str()
        .unwrap()
        .to_string();

    let mut xev = Command::new("xev")
        .arg("-display")
        .arg(&display)
        .args(["-geometry", "200x200+50+50"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn xev");

    thread::sleep(Duration::from_millis(1000));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_click",
            "arguments": { "display_id": id, "x": 150, "y": 150, "button": "right" },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_click",
            "arguments": { "display_id": id, "x": 150, "y": 150, "count": 2 },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    let _ = xev.kill();
    let output = xev.wait_with_output().expect("xev wait failed");
    let text = String::from_utf8_lossy(&output.stdout);

    let presses = text.matches("ButtonPress").count();
    assert!(
        text.contains("button 3"),
        "right click did not emit button 3: {text}"
    );
    assert!(
        presses >= 3,
        "expected right press + double left presses, got {presses}: {text}"
    );

    s.destroy_display(&id);
}

#[test]
fn keyboard_input_reaches_xev() {
    let mut s = McpSession::new();
    let id = s.create_display();
    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    )["result"]["display"]
        .as_str()
        .unwrap()
        .to_string();

    let mut xev = Command::new("xev")
        .arg("-display")
        .arg(&display)
        .args(["-geometry", "200x200+50+50"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn xev");

    thread::sleep(Duration::from_millis(1000));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_type",
            "arguments": { "display_id": id, "text": "hi" },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_key",
            "arguments": { "display_id": id, "key": "Return" },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    let _ = xev.kill();
    let output = xev.wait_with_output().expect("xev wait failed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("KeyPress"), "no key press detected: {text}");
    assert!(
        text.contains("KeyRelease"),
        "no key release detected: {text}"
    );

    s.destroy_display(&id);
}

#[test]
fn move_and_get_cursor_position() {
    let mut s = McpSession::new();
    let id = s.create_display();

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_move",
            "arguments": { "display_id": id, "x": 123, "y": 456 },
        }),
    );

    let pos = s.call(
        "tools/call",
        json!({
            "name": "lxs_input_get_cursor_position",
            "arguments": { "display_id": id },
        }),
    );
    assert_eq!(pos["result"]["x"].as_i64().unwrap(), 123);
    assert_eq!(pos["result"]["y"].as_i64().unwrap(), 456);

    s.destroy_display(&id);
}

#[test]
fn scroll_reaches_xev() {
    let mut s = McpSession::new();
    let id = s.create_display();
    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    )["result"]["display"]
        .as_str()
        .unwrap()
        .to_string();

    let mut xev = Command::new("xev")
        .arg("-display")
        .arg(&display)
        .args(["-geometry", "200x200+50+50"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn xev");

    thread::sleep(Duration::from_millis(1000));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_click",
            "arguments": { "display_id": id, "x": 150, "y": 150 },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_scroll",
            "arguments": { "display_id": id, "dx": 0, "dy": 2 },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_scroll",
            "arguments": { "display_id": id, "dx": -1, "dy": 0 },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    let _ = xev.kill();
    let output = xev.wait_with_output().expect("xev wait failed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("button 5"),
        "vertical scroll did not emit button 5: {text}"
    );
    assert!(
        text.contains("button 6"),
        "horizontal scroll did not emit button 6: {text}"
    );

    s.destroy_display(&id);
}

#[test]
fn drag_reaches_xev() {
    let mut s = McpSession::new();
    let id = s.create_display();
    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    )["result"]["display"]
        .as_str()
        .unwrap()
        .to_string();

    let mut xev = Command::new("xev")
        .arg("-display")
        .arg(&display)
        .args(["-geometry", "300x300+50+50"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn xev");

    thread::sleep(Duration::from_millis(1000));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_input_drag",
            "arguments": { "display_id": id, "x1": 100, "y1": 100, "x2": 200, "y2": 200 },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    let _ = xev.kill();
    let output = xev.wait_with_output().expect("xev wait failed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("ButtonPress"),
        "drag did not start with button press: {text}"
    );
    assert!(
        text.contains("MotionNotify"),
        "drag did not generate motion: {text}"
    );
    assert!(
        text.contains("ButtonRelease"),
        "drag did not end with button release: {text}"
    );

    s.destroy_display(&id);
}

#[test]
fn input_operations_do_not_error() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let ops = [
        (
            "lxs_input_move",
            json!({ "display_id": id, "x": 10, "y": 20 }),
        ),
        (
            "lxs_input_click",
            json!({ "display_id": id, "x": 10, "y": 20 }),
        ),
        (
            "lxs_input_scroll",
            json!({ "display_id": id, "dx": 0, "dy": 1 }),
        ),
        (
            "lxs_input_drag",
            json!({ "display_id": id, "x1": 10, "y1": 20, "x2": 30, "y2": 40 }),
        ),
        ("lxs_input_get_cursor_position", json!({ "display_id": id })),
        ("lxs_input_type", json!({ "display_id": id, "text": "ok" })),
        (
            "lxs_input_key",
            json!({ "display_id": id, "key": "Escape" }),
        ),
    ];

    for (name, args) in ops {
        let resp = s.call("tools/call", json!({ "name": name, "arguments": args }));
        assert!(resp["error"].is_null(), "{name} returned error: {resp}",);
    }

    s.destroy_display(&id);
}

#[test]
fn screenshot_is_valid_png() {
    let mut s = McpSession::new();
    let id = s.create_display();

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
    assert!(data.len() > 100);

    s.destroy_display(&id);
}

#[test]
fn window_screenshot_is_valid_png() {
    let mut s = McpSession::new();
    let id = s.create_display();

    s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": { "display_id": id, "command": "xterm", "args": [] },
        }),
    );
    thread::sleep(Duration::from_millis(1000));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let windows = overview["result"]["windows"].as_array().unwrap();
    assert!(!windows.is_empty(), "no windows found");

    let mut shot = None;
    for window in windows.iter().filter(|w| {
        let w_ = w["bounds"]["w"].as_u64().unwrap_or(0);
        let h = w["bounds"]["h"].as_u64().unwrap_or(0);
        w_ > 0 && h > 0
    }) {
        let window_id = window["window_id"].as_u64().unwrap() as u32;
        let resp = s.call(
            "tools/call",
            json!({
                "name": "lxs_capture_window",
                "arguments": { "display_id": id, "window_id": window_id },
            }),
        );
        if resp["error"].is_null() {
            shot = Some(resp);
            break;
        }
    }
    let shot = shot.expect("no capturable window found");
    assert_eq!(shot["result"]["mimeType"], "image/png");

    let data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        shot["result"]["data"].as_str().unwrap(),
    )
    .unwrap();
    assert!(data.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));

    s.destroy_display(&id);
}

#[test]
fn clipboard_roundtrip_works() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let text = "lxs-clipboard-test-42";
    s.call(
        "tools/call",
        json!({
            "name": "lxs_clipboard_set",
            "arguments": { "display_id": id, "text": text },
        }),
    );

    let got = s.call(
        "tools/call",
        json!({
            "name": "lxs_clipboard_get",
            "arguments": { "display_id": id },
        }),
    );
    assert_eq!(got["result"]["text"].as_str().unwrap(), text);

    s.destroy_display(&id);
}

#[test]
fn wait_sleeps_requested_duration() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let start = Instant::now();
    s.call(
        "tools/call",
        json!({
            "name": "lxs_wait",
            "arguments": { "ms": 300 },
        }),
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(250),
        "wait returned too early"
    );

    s.destroy_display(&id);
}

#[test]
fn window_set_frame_changes_bounds() {
    let mut s = McpSession::new();
    let id = s.create_display();

    s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": { "display_id": id, "command": "xterm", "args": ["-geometry", "200x200+0+0"] },
        }),
    );
    thread::sleep(Duration::from_millis(800));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let windows = overview["result"]["windows"].as_array().unwrap();
    let window = windows
        .iter()
        .filter(|w| {
            let w_ = w["bounds"]["w"].as_u64().unwrap_or(0);
            let h = w["bounds"]["h"].as_u64().unwrap_or(0);
            w_ > 0 && h > 0
        })
        .max_by_key(|w| {
            let w_ = w["bounds"]["w"].as_u64().unwrap_or(0);
            let h = w["bounds"]["h"].as_u64().unwrap_or(0);
            w_ * h
        })
        .expect("no window with bounds found");
    let window_id = window["window_id"].as_u64().unwrap() as u32;

    s.call(
        "tools/call",
        json!({
            "name": "lxs_window_set_frame",
            "arguments": { "display_id": id, "window_id": window_id, "x": 50, "y": 60, "width": 250, "height": 150 },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let updated = overview["result"]["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["window_id"].as_u64().unwrap() as u32 == window_id)
        .cloned()
        .expect("window disappeared");
    let x = updated["bounds"]["x"].as_i64().unwrap();
    let y = updated["bounds"]["y"].as_i64().unwrap();
    let w = updated["bounds"]["w"].as_u64().unwrap();
    let h = updated["bounds"]["h"].as_u64().unwrap();
    assert_eq!(x, 50, "x did not change");
    assert_eq!(y, 60, "y did not change");
    assert!(w > 0, "width invalid");
    assert!(h > 0, "height invalid");

    s.destroy_display(&id);
}

#[test]
fn window_focus_changes_active_window() {
    let mut s = McpSession::new();
    let id = s.create_display();
    let display = s.call(
        "tools/call",
        json!({"name": "lxs_display_info", "arguments": {"display_id": id}}),
    )["result"]["display"]
        .as_str()
        .unwrap()
        .to_string();

    s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": { "display_id": id, "command": "xterm", "args": ["-geometry", "200x200+0+0"] },
        }),
    );
    s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": { "display_id": id, "command": "xterm", "args": ["-geometry", "200x200+300+0"] },
        }),
    );
    thread::sleep(Duration::from_millis(800));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let windows = overview["result"]["windows"].as_array().unwrap();
    assert!(windows.len() >= 2, "expected at least 2 windows");
    let target_id = windows[1]["window_id"].as_u64().unwrap() as u32;

    let active_before = read_active_window(&display);

    s.call(
        "tools/call",
        json!({
            "name": "lxs_window_focus",
            "arguments": { "display_id": id, "window_id": target_id },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    let active_after = read_active_window(&display);
    assert_ne!(active_after, active_before, "active window did not change");
    assert!(active_after != 0, "active window is invalid");

    let title = Command::new("xprop")
        .arg("-display")
        .arg(&display)
        .arg("-id")
        .arg(format!("{}", active_after))
        .arg("WM_NAME")
        .output()
        .expect("xprop failed");
    let title_text = String::from_utf8_lossy(&title.stdout);
    assert!(
        title_text.contains("WM_NAME"),
        "active window has no WM_NAME: {title_text}"
    );

    s.destroy_display(&id);
}

fn read_active_window(display: &str) -> u32 {
    let output = Command::new("xprop")
        .arg("-display")
        .arg(display)
        .arg("-root")
        .arg("_NET_ACTIVE_WINDOW")
        .output()
        .expect("xprop failed");
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .next_back()
        .and_then(|s| s.strip_prefix("0x"))
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .unwrap_or(0)
}

#[test]
fn window_close_removes_window() {
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

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let windows = overview["result"]["windows"].as_array().unwrap();
    let window = windows
        .iter()
        .filter(|w| {
            let w_ = w["bounds"]["w"].as_u64().unwrap_or(0);
            let h = w["bounds"]["h"].as_u64().unwrap_or(0);
            w_ > 0 && h > 0
        })
        .max_by_key(|w| {
            let w_ = w["bounds"]["w"].as_u64().unwrap_or(0);
            let h = w["bounds"]["h"].as_u64().unwrap_or(0);
            w_ * h
        })
        .expect("xterm window not found")
        .clone();
    let window_id = window["window_id"].as_u64().unwrap() as u32;

    s.call(
        "tools/call",
        json!({
            "name": "lxs_window_close",
            "arguments": { "display_id": id, "window_id": window_id },
        }),
    );
    thread::sleep(Duration::from_millis(800));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let still_present = overview["result"]["windows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["window_id"].as_u64().unwrap() as u32 == window_id);
    assert!(!still_present, "window still present after close");

    s.destroy_display(&id);
}

#[test]
fn atspi_tree_is_populated_for_chromium() {
    let mut s = McpSession::new();
    let id = s.create_display();

    if Command::new("chromium").arg("--version").output().is_err() {
        eprintln!("chromium not installed, skipping");
        s.destroy_display(&id);
        return;
    }

    let launch = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": {
                "display_id": id,
                "command": "chromium",
                "args": ["--no-sandbox", "--disable-gpu", "--no-first-run", "--force-renderer-accessibility", "about:blank"]
            },
        }),
    );
    assert!(launch["error"].is_null(), "launch failed: {launch}");
    let pid = launch["result"]["pid"].as_u64().unwrap() as u32;

    thread::sleep(Duration::from_millis(4000));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let windows = overview["result"]["windows"].as_array().unwrap();
    let window = find_chromium_window(windows, pid);
    let window_id = window["window_id"].as_u64().unwrap() as u32;

    s.call(
        "tools/call",
        json!({
            "name": "lxs_window_focus",
            "arguments": { "display_id": id, "window_id": window_id },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    let mut elements = None;
    for _ in 0..5 {
        let state = s.call(
            "tools/call",
            json!({
                "name": "lxs_get_window_state",
                "arguments": { "display_id": id, "pid": pid, "window_id": window_id, "include_tree": true },
            }),
        );
        if let Some(arr) = state["result"]["tree"]["elements"].as_array() {
            if !arr.is_empty() {
                elements = Some(arr.clone());
                break;
            }
        }
        thread::sleep(Duration::from_millis(1000));
    }
    let elements = elements.expect("chromium window state tree was empty after retries");
    assert!(!elements.is_empty(), "AT-SPI tree was empty");

    let _ = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_terminate",
            "arguments": { "display_id": id, "pid": pid },
        }),
    );
    s.destroy_display(&id);
}

#[test]
fn click_element_and_set_value_work_with_gtk() {
    let mut s = McpSession::new();
    let id = s.create_display();

    let py = std::env::temp_dir().join("lxs_atspi_test.py");
    {
        let mut f = File::create(&py).expect("create python test app");
        f.write_all(
            br#"import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, GLib

class App(Gtk.Window):
    def __init__(self):
        super().__init__(title='lxs_atspi_test')
        self.set_default_size(300, 120)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        self.add(box)
        self.entry = Gtk.Entry()
        self.entry.set_text('')
        self.entry.set_name('test_entry')
        box.pack_start(self.entry, True, True, 0)
        btn = Gtk.Button(label='Click')
        btn.connect('clicked', self.on_click)
        box.pack_start(btn, True, True, 0)
        self.label = Gtk.Label(label='ready')
        box.pack_start(self.label, True, True, 0)
        self.connect('destroy', Gtk.main_quit)
        self.show_all()

    def on_click(self, _):
        text = self.entry.get_text()
        self.label.set_text(f'clicked-{text}')

if __name__ == '__main__':
    app = App()
    GLib.timeout_add_seconds(30, Gtk.main_quit)
    Gtk.main()
"#,
        )
        .expect("write python app");
    }

    let launch = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_launch",
            "arguments": {
                "display_id": id,
                "command": "python3",
                "args": [py.to_str().unwrap()]
            },
        }),
    );
    assert!(launch["error"].is_null(), "launch failed: {launch}");
    let pid = launch["result"]["pid"].as_u64().unwrap() as u32;

    thread::sleep(Duration::from_millis(2000));

    let overview = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_desktop_overview",
            "arguments": { "display_id": id },
        }),
    );
    let windows = overview["result"]["windows"].as_array().unwrap();
    let window = windows
        .iter()
        .find(|w| w["pid"].as_u64().map(|p| p as u32 == pid).unwrap_or(false))
        .expect("gtk window not found")
        .clone();
    let window_id = window["window_id"].as_u64().unwrap() as u32;

    s.call(
        "tools/call",
        json!({
            "name": "lxs_window_focus",
            "arguments": { "display_id": id, "window_id": window_id },
        }),
    );
    thread::sleep(Duration::from_millis(300));

    let mut elements = None;
    for _ in 0..5 {
        let state = s.call(
            "tools/call",
            json!({
                "name": "lxs_get_window_state",
                "arguments": { "display_id": id, "pid": pid, "window_id": window_id, "include_tree": true },
            }),
        );
        if let Some(arr) = state["result"]["tree"]["elements"].as_array() {
            if !arr.is_empty() {
                elements = Some(arr.clone());
                break;
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    let elements = elements.expect("window state tree was empty after retries");

    let entry_idx = elements
        .iter()
        .position(|e| {
            let role = e["role"].as_str().unwrap_or("");
            role == "text" || role == "entry"
        })
        .expect("entry element not found");

    let button_idx = elements
        .iter()
        .position(|e| {
            let role = e["role"].as_str().unwrap_or("").to_lowercase();
            let name = e["name"].as_str().unwrap_or("");
            role.contains("button") && name == "Click"
        })
        .expect("button element not found");

    let set_resp = s.call(
        "tools/call",
        json!({
            "name": "lxs_set_value",
            "arguments": { "display_id": id, "pid": pid, "index": entry_idx, "value": "hello" },
        }),
    );
    assert!(set_resp["error"].is_null(), "set_value failed: {set_resp}");
    thread::sleep(Duration::from_millis(200));

    s.call(
        "tools/call",
        json!({
            "name": "lxs_click_element",
            "arguments": { "display_id": id, "pid": pid, "index": button_idx, "button": "left" },
        }),
    );
    thread::sleep(Duration::from_millis(500));

    let state2 = s.call(
        "tools/call",
        json!({
            "name": "lxs_get_window_state",
            "arguments": { "display_id": id, "pid": pid, "window_id": window_id, "include_tree": true },
        }),
    );
    let elements2 = state2["result"]["tree"]["elements"].as_array().unwrap();
    let result_text: String = elements2
        .iter()
        .filter_map(|e| e["name"].as_str())
        .find(|n| n.starts_with("clicked-"))
        .map(|s| s.to_string())
        .expect("button click did not update label");
    assert_eq!(result_text, "clicked-hello");

    let _ = s.call(
        "tools/call",
        json!({
            "name": "lxs_app_terminate",
            "arguments": { "display_id": id, "pid": pid },
        }),
    );
    s.destroy_display(&id);
}

fn find_chromium_window(windows: &[Value], _pid: u32) -> &Value {
    windows
        .iter()
        .find(|w| w["pid"].as_u64().map(|p| p as u32 == _pid).unwrap_or(false))
        .or_else(|| {
            windows.iter().find(|w| {
                w["title"]
                    .as_str()
                    .map(|t| t.to_lowercase().contains("chromium"))
                    .unwrap_or(false)
            })
        })
        .or_else(|| {
            windows.iter().find(|w| {
                w["app_name"]
                    .as_str()
                    .map(|a| a.to_lowercase().contains("chromium"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or_else(|| panic!("chromium window not found; windows: {windows:?}"))
}
