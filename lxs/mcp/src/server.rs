use std::io;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use lxs_core::{Driver, LxsError, MouseButton, Rect};
use lxs_runtime::{Backend, Display, DisplayConfig, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub struct McpServer {
    runtime: Arc<Runtime>,
    displays: Mutex<Vec<Arc<Mutex<Display>>>>,
}

#[derive(Deserialize)]
#[serde(crate = "serde")]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
#[serde(crate = "serde")]
struct Response {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorDetail>,
}

#[derive(Serialize)]
#[serde(crate = "serde")]
struct ErrorDetail {
    code: i32,
    message: String,
}

impl McpServer {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            runtime,
            displays: Mutex::new(Vec::new()),
        }
    }

    pub async fn run_stdio(&self) -> io::Result<()> {
        let stdin = BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();
        let mut lines = stdin.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            let req: Request = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(_) => continue,
            };

            if req.method.starts_with("notifications/") {
                continue;
            }

            let resp = match req.method.as_str() {
                "initialize" => Response {
                    jsonrpc: "2.0".into(),
                    id: req.id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "linux-x11-sandbox", "version": "0.1.0" }
                    })),
                    error: None,
                },
                "tools/list" => Response {
                    jsonrpc: "2.0".into(),
                    id: req.id,
                    result: Some(json!({ "tools": tool_definitions() })),
                    error: None,
                },
                "tools/call" => self.handle_tool_call(req.id, &req.params).await,
                _ => error_response(req.id, -32601, "method not found"),
            };

            let msg = serde_json::to_string(&resp).unwrap();
            stdout.write_all(msg.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    async fn handle_tool_call(&self, id: Option<Value>, params: &Value) -> Response {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        let result = match name {
            "lxs_display_create" => self.create_display(args).await,
            "lxs_display_destroy" => self.destroy_display(args).await,
            "lxs_app_launch" => self.app_launch(args).await,
            "lxs_app_terminate" => self.app_terminate(args).await,
            "lxs_app_list" => self.app_list(args).await,
            "lxs_input_click" => self.click(args).await,
            "lxs_input_move" => self.move_mouse(args).await,
            "lxs_input_type" => self.input_type(args).await,
            "lxs_input_key" => self.input_key(args).await,
            "lxs_capture_screenshot" => self.screenshot(args).await,
            "lxs_capture_region" => self.screenshot_region(args).await,
            "lxs_display_info" => self.display_info(args).await,
            "lxs_state_window" => self.state_window(args).await,
            "lxs_state_tree" => self.state_tree(args).await,
            "lxs_state_element_bounds" => self.element_bounds(args).await,
            "lxs_perform_action" => self.perform_action(args).await,
            "lxs_input_scroll" => self.scroll(args).await,
            "lxs_input_drag" => self.drag(args).await,
            "lxs_window_focus" => self.window_focus(args).await,
            "lxs_window_raise" => self.window_raise(args).await,
            "lxs_window_resize" => self.window_resize(args).await,
            "lxs_window_move" => self.window_move(args).await,
            "lxs_clipboard_get" => self.clipboard_get(args).await,
            "lxs_clipboard_set" => self.clipboard_set(args).await,
            "lxs_input_get_cursor_position" => self.get_cursor_position(args).await,
            "lxs_window_list" => self.window_list(args).await,
            "lxs_wait" => self.wait(args).await,
            _ => Err(LxsError::InvalidArgument(format!("unknown tool: {}", name))),
        };

        match result {
            Ok(value) => Response {
                jsonrpc: "2.0".into(),
                id,
                result: Some(value),
                error: None,
            },
            Err(e) => error_response(id, -32603, &e.to_string()),
        }
    }

    async fn create_display(&self, args: &Value) -> Result<Value, LxsError> {
        let mut config = DisplayConfig::default();
        if let Some(backend) = args["backend"].as_str() {
            config.backend = match backend {
                "xvfb" => Backend::Xvfb,
                "xephyr" => Backend::Xephyr,
                _ => {
                    return Err(LxsError::InvalidArgument(format!(
                        "unknown backend: {backend}"
                    )))
                }
            };
        }

        let display = self.runtime.create_display(config).await?;
        let id = display.id().to_string();
        let display_str = display.display().to_string();
        self.displays
            .lock()
            .await
            .push(Arc::new(Mutex::new(display)));
        Ok(json!({
            "display_id": id,
            "display": display_str
        }))
    }

    async fn destroy_display(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let display = self.find_display(id).await?;
        display.lock().await.destroy().await?;

        let mut displays = self.displays.lock().await;
        let mut kept = Vec::new();
        for d in displays.drain(..) {
            if d.lock().await.id() != id {
                kept.push(d);
            }
        }
        *displays = kept;
        Ok(json!({ "success": true }))
    }

    async fn app_launch(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let command = args["command"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("command required".into()))?;
        let args_vec: Vec<String> = args["args"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let arg_refs: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();

        let display = self.find_display(id).await?;
        let pid = display.lock().await.launch_app(command, &arg_refs).await?;
        Ok(json!({ "pid": pid }))
    }

    async fn app_terminate(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let pid = args["pid"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))?
            as u32;

        let display = self.find_display(id).await?;
        display.lock().await.terminate_app(pid).await?;
        Ok(json!({ "success": true }))
    }

    async fn app_list(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let display = self.find_display(id).await?;
        let pids = display.lock().await.list_apps();
        Ok(json!({ "pids": pids }))
    }

    async fn click(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x = args["x"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
        let y = args["y"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;
        let button = match args["button"].as_str().unwrap_or("left") {
            "left" => MouseButton::Left,
            "middle" => MouseButton::Middle,
            "right" => MouseButton::Right,
            b => return Err(LxsError::InvalidArgument(format!("unknown button: {b}"))),
        };
        let count = args["count"].as_u64().unwrap_or(1) as u32;

        let driver = self.find_driver(id).await?;
        driver.click(x, y, button, count).await?;
        Ok(json!({ "success": true }))
    }

    async fn move_mouse(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x = args["x"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
        let y = args["y"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;

        let driver = self.find_driver(id).await?;
        driver.move_mouse(x, y).await?;
        Ok(json!({ "success": true }))
    }

    async fn input_type(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let text = args["text"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("text required".into()))?;

        let driver = self.find_driver(id).await?;
        driver.type_text(text).await?;
        Ok(json!({ "success": true }))
    }

    async fn input_key(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let key = args["key"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("key required".into()))?;
        let modifiers: Vec<String> = args["modifiers"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let mod_refs: Vec<&str> = modifiers.iter().map(|s| s.as_str()).collect();

        let driver = self.find_driver(id).await?;
        driver.key(key, &mod_refs).await?;
        Ok(json!({ "success": true }))
    }

    async fn screenshot(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        let shot = driver.screenshot().await?;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &shot.data);
        Ok(json!({
            "mimeType": "image/png",
            "data": b64
        }))
    }

    async fn screenshot_region(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x = args["x"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
        let y = args["y"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;
        let w = args["w"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("w required".into()))? as u32;
        let h = args["h"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("h required".into()))? as u32;

        let driver = self.find_driver(id).await?;
        let shot = driver.screenshot_region(Rect { x, y, w, h }).await?;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &shot.data);
        Ok(json!({
            "mimeType": "image/png",
            "data": b64
        }))
    }

    async fn scroll(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let dx = args["dx"].as_i64().unwrap_or(0) as i32;
        let dy = args["dy"].as_i64().unwrap_or(0) as i32;

        let driver = self.find_driver(id).await?;
        driver.scroll(dx, dy).await?;
        Ok(json!({ "success": true }))
    }

    async fn drag(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x1 = args["x1"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("x1 required".into()))?
            as i32;
        let y1 = args["y1"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("y1 required".into()))?
            as i32;
        let x2 = args["x2"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("x2 required".into()))?
            as i32;
        let y2 = args["y2"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("y2 required".into()))?
            as i32;

        let driver = self.find_driver(id).await?;
        driver.drag(x1, y1, x2, y2, MouseButton::Left).await?;
        Ok(json!({ "success": true }))
    }

    async fn window_focus(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        driver.focus_window().await?;
        Ok(json!({ "success": true }))
    }

    async fn window_raise(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        driver.raise_window().await?;
        Ok(json!({ "success": true }))
    }

    async fn window_resize(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let width = args["width"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("width required".into()))?
            as u32;
        let height = args["height"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("height required".into()))?
            as u32;

        let driver = self.find_driver(id).await?;
        driver.resize_window(width, height).await?;
        Ok(json!({ "success": true }))
    }

    async fn window_move(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x = args["x"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
        let y = args["y"]
            .as_i64()
            .ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;

        let driver = self.find_driver(id).await?;
        driver.move_window(x, y).await?;
        Ok(json!({ "success": true }))
    }

    async fn clipboard_get(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        let text = driver.clipboard_get().await?;
        Ok(json!({ "text": text }))
    }

    async fn clipboard_set(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let text = args["text"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("text required".into()))?;
        let driver = self.find_driver(id).await?;
        driver.clipboard_set(text).await?;
        Ok(json!({ "success": true }))
    }

    async fn get_cursor_position(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        let (x, y) = driver.get_cursor_position().await?;
        Ok(json!({ "x": x, "y": y }))
    }

    async fn window_list(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        let windows = driver.list_windows().await?;
        let items: Vec<Value> = windows
            .iter()
            .map(|w| json!({ "id": w.id, "title": w.title }))
            .collect();
        Ok(json!({ "windows": items }))
    }

    async fn wait(&self, args: &Value) -> Result<Value, LxsError> {
        let ms = args["ms"].as_u64().unwrap_or(1000);
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        Ok(json!({ "success": true }))
    }

    async fn display_info(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let display = self.find_display(id).await?;
        let info = display.lock().await.info();
        Ok(json!({
            "display": info.display,
            "width": info.width,
            "height": info.height,
            "app_count": info.app_count,
        }))
    }

    async fn state_window(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id).await?;
        let state = driver.window_state().await?;
        Ok(json!({ "title": state.title }))
    }

    async fn state_tree(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let pid = args["pid"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))?
            as u32;

        let driver = self.find_driver(id).await?;
        let tree = driver.accessibility_tree(Some(pid)).await?;
        let elements: Vec<Value> = tree
            .elements
            .iter()
            .map(|e| {
                json!({
                    "index": e.index,
                    "role": e.role,
                    "name": e.name,
                    "actions": e.actions,
                })
            })
            .collect();
        Ok(json!({ "elements": elements }))
    }

    async fn element_bounds(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let pid = args["pid"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))?
            as u32;
        let index = args["index"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("index required".into()))?
            as usize;

        let driver = self.find_driver(id).await?;
        let bounds = driver.element_bounds(pid, index).await?;
        Ok(json!({
            "x": bounds.x,
            "y": bounds.y,
            "w": bounds.w,
            "h": bounds.h,
        }))
    }

    async fn perform_action(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let pid = args["pid"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))?
            as u32;
        let index = args["index"]
            .as_u64()
            .ok_or_else(|| LxsError::InvalidArgument("index required".into()))?
            as usize;
        let action = args["action"]
            .as_str()
            .ok_or_else(|| LxsError::InvalidArgument("action required".into()))?;

        let driver = self.find_driver(id).await?;
        driver.perform_action(pid, index, action).await?;
        Ok(json!({ "success": true }))
    }

    async fn find_driver(&self, id: &str) -> Result<Arc<dyn Driver>, LxsError> {
        let display = self.find_display(id).await?;
        let driver = display.lock().await.driver();
        Ok(driver)
    }

    async fn find_display(&self, id: &str) -> Result<Arc<Mutex<Display>>, LxsError> {
        let displays = self.displays.lock().await;
        for display in displays.iter() {
            if display.lock().await.id() == id {
                return Ok(Arc::clone(display));
            }
        }
        Err(LxsError::DisplayNotFound(id.into()))
    }
}

fn error_response(id: Option<Value>, code: i32, message: &str) -> Response {
    Response {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(ErrorDetail {
            code,
            message: message.into(),
        }),
    }
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "lxs_display_create",
            "description": "Create a new X11 display (backend: xvfb or xephyr)",
            "inputSchema": { "type": "object", "properties": { "backend": { "type": "string", "enum": ["xvfb", "xephyr"] } } }
        }),
        json!({
            "name": "lxs_display_destroy",
            "description": "Destroy an X11 display",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_app_launch",
            "description": "Launch an application on a display",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "command": { "type": "string" }, "args": { "type": "array", "items": { "type": "string" } } }, "required": ["display_id", "command"] }
        }),
        json!({
            "name": "lxs_app_terminate",
            "description": "Terminate an application by PID",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" } }, "required": ["display_id", "pid"] }
        }),
        json!({
            "name": "lxs_app_list",
            "description": "List running applications on a display",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_input_click",
            "description": "Click at screen coordinates",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" }, "button": { "type": "string", "enum": ["left", "middle", "right"] }, "count": { "type": "integer", "minimum": 1 } }, "required": ["display_id", "x", "y"] }
        }),
        json!({
            "name": "lxs_input_move",
            "description": "Move the mouse cursor",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" } }, "required": ["display_id", "x", "y"] }
        }),
        json!({
            "name": "lxs_input_type",
            "description": "Type text",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "text": { "type": "string" } }, "required": ["display_id", "text"] }
        }),
        json!({
            "name": "lxs_input_key",
            "description": "Press a key or key combination",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "key": { "type": "string" }, "modifiers": { "type": "array", "items": { "type": "string" } } }, "required": ["display_id", "key"] }
        }),
        json!({
            "name": "lxs_capture_screenshot",
            "description": "Take a screenshot",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_state_window",
            "description": "Get the active window title",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_state_tree",
            "description": "Walk the AT-SPI accessibility tree for a process",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" } }, "required": ["display_id", "pid"] }
        }),
        json!({
            "name": "lxs_state_element_bounds",
            "description": "Get screen bounds of an indexed accessibility element",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "index": { "type": "integer" } }, "required": ["display_id", "pid", "index"] }
        }),
        json!({
            "name": "lxs_perform_action",
            "description": "Perform a named AT-SPI action on an indexed element",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "index": { "type": "integer" }, "action": { "type": "string" } }, "required": ["display_id", "pid", "index", "action"] }
        }),
        json!({
            "name": "lxs_input_scroll",
            "description": "Scroll by a delta",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "dx": { "type": "integer" }, "dy": { "type": "integer" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_input_get_cursor_position",
            "description": "Get the current mouse cursor position",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_input_drag",
            "description": "Drag from (x1, y1) to (x2, y2)",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x1": { "type": "integer" }, "y1": { "type": "integer" }, "x2": { "type": "integer" }, "y2": { "type": "integer" } }, "required": ["display_id", "x1", "y1", "x2", "y2"] }
        }),
        json!({
            "name": "lxs_window_focus",
            "description": "Focus the active window",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_window_raise",
            "description": "Raise the active window to the top",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_window_resize",
            "description": "Resize the active window",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "width": { "type": "integer" }, "height": { "type": "integer" } }, "required": ["display_id", "width", "height"] }
        }),
        json!({
            "name": "lxs_window_move",
            "description": "Move the active window",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" } }, "required": ["display_id", "x", "y"] }
        }),
        json!({
            "name": "lxs_clipboard_get",
            "description": "Get text from the clipboard",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_clipboard_set",
            "description": "Set text on the clipboard",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "text": { "type": "string" } }, "required": ["display_id", "text"] }
        }),
        json!({
            "name": "lxs_window_list",
            "description": "List top-level windows",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_wait",
            "description": "Wait for a specified duration in milliseconds",
            "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer", "minimum": 0 } }, "required": [] }
        }),
        json!({
            "name": "lxs_capture_region",
            "description": "Take a screenshot of a region",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" }, "w": { "type": "integer" }, "h": { "type": "integer" } }, "required": ["display_id", "x", "y", "w", "h"] }
        }),
        json!({
            "name": "lxs_display_info",
            "description": "Get display metadata",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
    ]
}
