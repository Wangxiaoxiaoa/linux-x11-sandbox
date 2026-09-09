use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

use lxs_core::{Driver, LxsError, MouseButton};
use lxs_runtime::{Display, DisplayConfig, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub struct McpServer {
    runtime: Arc<Runtime>,
    displays: Mutex<Vec<Arc<Mutex<Display>>>>,
    tokio: tokio::runtime::Runtime,
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
            tokio: tokio::runtime::Runtime::new().unwrap(),
        }
    }

    pub fn run_stdio(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout().lock();

        for line in stdin.lock().lines() {
            let line = line?;
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
                "tools/call" => self.handle_tool_call(req.id, &req.params),
                _ => error_response(req.id, -32601, "method not found"),
            };

            writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap())?;
            stdout.flush()?;
        }

        Ok(())
    }

    fn handle_tool_call(&self, id: Option<Value>, params: &Value) -> Response {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        let result = self.tokio.block_on(async {
            match name {
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
                _ => Err(LxsError::InvalidArgument(format!("unknown tool: {}", name))),
            }
        });

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

    async fn create_display(&self, _args: &Value) -> Result<Value, LxsError> {
        let display = self.runtime.create_display(DisplayConfig::default()).await?;
        let id = display.id().to_string();
        let display_str = display.display().to_string();
        self.displays.lock().unwrap().push(Arc::new(Mutex::new(display)));
        Ok(json!({
            "display_id": id,
            "display": display_str
        }))
    }

    async fn destroy_display(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let display = self.find_display(id)?;
        display.lock().unwrap().destroy().await?;

        let mut displays = self.displays.lock().unwrap();
        displays.retain(|d| d.lock().unwrap().id() != id);
        Ok(json!({ "success": true }))
    }

    async fn app_launch(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let command = args["command"].as_str().ok_or_else(|| LxsError::InvalidArgument("command required".into()))?;
        let args_vec: Vec<String> = args["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let arg_refs: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();

        let display = self.find_display(id)?;
        let pid = display.lock().unwrap().launch_app(command, &arg_refs).await?;
        Ok(json!({ "pid": pid }))
    }

    async fn app_terminate(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let pid = args["pid"].as_u64().ok_or_else(|| LxsError::InvalidArgument("pid required".into()))? as u32;

        let display = self.find_display(id)?;
        display.lock().unwrap().terminate_app(pid).await?;
        Ok(json!({ "success": true }))
    }

    async fn app_list(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let display = self.find_display(id)?;
        let pids = display.lock().unwrap().list_apps();
        Ok(json!({ "pids": pids }))
    }

    async fn click(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x = args["x"].as_i64().ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
        let y = args["y"].as_i64().ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;

        let driver = self.find_driver(id)?;
        driver.click(x, y, MouseButton::Left, 1).await?;
        Ok(json!({ "success": true }))
    }

    async fn move_mouse(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let x = args["x"].as_i64().ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
        let y = args["y"].as_i64().ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;

        let driver = self.find_driver(id)?;
        driver.move_mouse(x, y).await?;
        Ok(json!({ "success": true }))
    }

    async fn input_type(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let text = args["text"].as_str().ok_or_else(|| LxsError::InvalidArgument("text required".into()))?;

        let driver = self.find_driver(id)?;
        driver.type_text(text).await?;
        Ok(json!({ "success": true }))
    }

    async fn input_key(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let key = args["key"].as_str().ok_or_else(|| LxsError::InvalidArgument("key required".into()))?;
        let modifiers: Vec<String> = args["modifiers"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let mod_refs: Vec<&str> = modifiers.iter().map(|s| s.as_str()).collect();

        let driver = self.find_driver(id)?;
        driver.key(key, &mod_refs).await?;
        Ok(json!({ "success": true }))
    }

    async fn screenshot(&self, args: &Value) -> Result<Value, LxsError> {
        let id = args["display_id"].as_str().ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
        let driver = self.find_driver(id)?;
        let shot = driver.screenshot().await?;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &shot.data);
        Ok(json!({
            "mimeType": "image/png",
            "data": b64
        }))
    }

    fn find_driver(&self, id: &str) -> Result<Arc<dyn Driver>, LxsError> {
        let display = self.find_display(id)?;
        let driver = display.lock().unwrap().driver();
        Ok(driver)
    }

    fn find_display(&self, id: &str) -> Result<Arc<Mutex<Display>>, LxsError> {
        let displays = self.displays.lock().unwrap();
        let display = displays.iter().find(|d| d.lock().unwrap().id() == id).ok_or_else(|| LxsError::DisplayNotFound(id.into()))?;
        Ok(Arc::clone(display))
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
            "description": "Create a new X11 display",
            "inputSchema": { "type": "object" }
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
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" } }, "required": ["display_id", "x", "y"] }
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
    ]
}
