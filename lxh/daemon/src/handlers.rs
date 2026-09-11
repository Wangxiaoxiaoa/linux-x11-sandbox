use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lxh_core::{Driver, LxhError, MouseButton};
use lxh_driver::DefaultDriver;
use lxh_runtime::{Backend, Display, DisplayConfig, Runtime};
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};

pub struct DaemonState {
    pub runtime: Arc<Runtime>,
    pub displays: Arc<RwLock<HashMap<String, Arc<Mutex<Display>>>>>,
    pub drivers: Arc<RwLock<HashMap<String, Arc<dyn Driver>>>>,
}

pub struct ClientSession {
    pub owned_displays: HashSet<String>,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            owned_displays: HashSet::new(),
        }
    }
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, LxhError> {
    args[key].as_str().ok_or_else(|| LxhError::InvalidArgument(format!("{key} required")))
}

fn require_i64(args: &Value, key: &str) -> Result<i64, LxhError> {
    args[key].as_i64().ok_or_else(|| LxhError::InvalidArgument(format!("{key} required")))
}

fn require_u64(args: &Value, key: &str) -> Result<u64, LxhError> {
    args[key].as_u64().ok_or_else(|| LxhError::InvalidArgument(format!("{key} required")))
}

pub async fn create_display(
    state: &DaemonState,
    session: &mut ClientSession,
    args: &Value,
) -> Result<Value, LxhError> {
    let mut config = DisplayConfig::default();
    if let Some(backend) = args["backend"].as_str() {
        config.backend = match backend {
            "xvfb" => Backend::Xvfb,
            "xephyr" => Backend::Xephyr,
            _ => {
                return Err(LxhError::InvalidArgument(format!(
                    "unknown backend: {backend}"
                )))
            }
        };
    }

    let display = state.runtime.create_display(config).await?;
    let id = display.id().to_string();
    let display_str = display.display().to_string();
    let driver = Arc::new(DefaultDriver::new(&display_str)?);

    state
        .displays
        .write()
        .await
        .insert(id.clone(), Arc::new(Mutex::new(display)));
    state.drivers.write().await.insert(id.clone(), driver);
    if !args["persistent"].as_bool().unwrap_or(false) {
        session.owned_displays.insert(id.clone());
    }

    Ok(json!({
        "display_id": id,
        "display": display_str
    }))
}

pub async fn attach_display(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let display_id = require_str(args, "display_id")?;

    let display = state.runtime.attach_display(display_id).await?;
    let id = display.id().to_string();
    let display_str = display.display().to_string();
    let driver = Arc::new(DefaultDriver::new(&display_str)?);

    state
        .displays
        .write()
        .await
        .insert(id.clone(), Arc::new(Mutex::new(display)));
    state.drivers.write().await.insert(id.clone(), driver);

    Ok(json!({
        "display_id": id,
        "display": display_str
    }))
}

pub async fn destroy_display(
    state: &DaemonState,
    session: &mut ClientSession,
    args: &Value,
) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;

    let display = find_display(state, id).await?;
    display.lock().await.destroy().await?;

    state.displays.write().await.remove(id);
    state.drivers.write().await.remove(id);
    session.owned_displays.remove(id);

    Ok(json!({ "success": true }))
}

pub async fn detach_display(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;

    let display = find_display(state, id).await?;
    if !display.lock().await.is_external() {
        return Err(LxhError::InvalidArgument(
            "cannot detach a harness display; use lxh_display_destroy".into(),
        ));
    }

    state.displays.write().await.remove(id);

    Ok(json!({ "success": true }))
}

pub async fn app_launch(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let command = require_str(args, "command")?;
    let args_vec: Vec<String> = args["args"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let arg_refs: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();

    let display = find_display(state, id).await?;
    let pid = display.lock().await.launch_app(command, &arg_refs).await?;
    Ok(json!({ "pid": pid }))
}

pub async fn app_terminate(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let pid = require_u64(args, "pid")? as u32;

    let display = find_display(state, id).await?;
    display.lock().await.terminate_app(pid).await?;
    Ok(json!({ "success": true }))
}

pub async fn click(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let x = require_i64(args, "x")? as i32;
    let y = require_i64(args, "y")? as i32;
    let button = match args["button"].as_str().unwrap_or("left") {
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    let count = args["count"].as_u64().unwrap_or(1) as u32;

    let driver = find_driver(state, id).await?;
    driver.click(x, y, button, count).await?;
    Ok(json!({ "success": true }))
}

pub async fn move_mouse(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let x = require_i64(args, "x")? as i32;
    let y = require_i64(args, "y")? as i32;

    let driver = find_driver(state, id).await?;
    driver.move_mouse(x, y).await?;
    Ok(json!({ "success": true }))
}

pub async fn input_type(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let text = require_str(args, "text")?;

    let driver = find_driver(state, id).await?;
    driver.type_text(text).await?;
    Ok(json!({ "success": true }))
}

pub async fn input_key(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let key = require_str(args, "key")?;
    let modifiers: Vec<String> = args["modifiers"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mod_refs: Vec<&str> = modifiers.iter().map(|s| s.as_str()).collect();

    let driver = find_driver(state, id).await?;
    driver.key(key, &mod_refs).await?;
    Ok(json!({ "success": true }))
}

pub async fn scroll(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let dx = args["dx"].as_i64().unwrap_or(0) as i32;
    let dy = args["dy"].as_i64().unwrap_or(0) as i32;

    let driver = find_driver(state, id).await?;
    driver.scroll(dx, dy).await?;
    Ok(json!({ "success": true }))
}

pub async fn drag(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let x1 = require_i64(args, "x1")? as i32;
    let y1 = require_i64(args, "y1")? as i32;
    let x2 = require_i64(args, "x2")? as i32;
    let y2 = require_i64(args, "y2")? as i32;

    let driver = find_driver(state, id).await?;
    driver.drag(x1, y1, x2, y2).await?;
    Ok(json!({ "success": true }))
}

pub async fn get_cursor_position(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let driver = find_driver(state, id).await?;
    let (x, y) = driver.get_cursor_position().await?;
    Ok(json!({ "x": x, "y": y }))
}

pub async fn screenshot(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let driver = find_driver(state, id).await?;
    let shot = driver.screenshot().await?;
    Ok(json!({
        "mimeType": "image/png",
        "data": encode_png(&shot.data)
    }))
}

pub async fn screenshot_window(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let window_id = require_u64(args, "window_id")?
        as u32;

    let driver = find_driver(state, id).await?;
    let shot = driver.screenshot_window(window_id).await?;
    Ok(json!({
        "mimeType": "image/png",
        "data": encode_png(&shot.data)
    }))
}

pub async fn window_focus(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let window_id = require_u64(args, "window_id")?
        as u32;

    let driver = find_driver(state, id).await?;
    driver.focus_window(window_id).await?;
    Ok(json!({ "success": true }))
}

pub async fn window_set_frame(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let window_id = require_u64(args, "window_id")?
        as u32;
    let x = require_i64(args, "x")? as i32;
    let y = require_i64(args, "y")? as i32;
    let width = require_u64(args, "width")? as u32;
    let height = require_u64(args, "height")?
        as u32;

    let driver = find_driver(state, id).await?;
    driver
        .set_window_frame(window_id, x, y, width, height)
        .await?;
    Ok(json!({ "success": true }))
}

pub async fn window_close(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let window_id = require_u64(args, "window_id")?
        as u32;

    let driver = find_driver(state, id).await?;
    driver.close_window(window_id).await?;
    Ok(json!({ "success": true }))
}

pub async fn click_element(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let pid = require_u64(args, "pid")? as u32;
    let index = require_u64(args, "index")?
        as usize;
    let button = match args["button"].as_str().unwrap_or("left") {
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => MouseButton::Left,
    };

    let driver = find_driver(state, id).await?;
    driver.click_element(pid, index, button).await?;
    Ok(json!({ "success": true }))
}

pub async fn wait(_state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let ms = require_u64(args, "ms")?;
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    Ok(json!({ "success": true }))
}

pub async fn clipboard_get(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let driver = find_driver(state, id).await?;
    let text = driver.clipboard_get().await?;
    Ok(json!({ "text": text }))
}

pub async fn clipboard_set(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let text = require_str(args, "text")?;
    let driver = find_driver(state, id).await?;
    driver.clipboard_set(text).await?;
    Ok(json!({ "success": true }))
}

pub async fn display_info(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let display = find_display(state, id).await?;
    let info = display.lock().await.info();
    Ok(json!({
        "display": info.display,
        "width": info.width,
        "height": info.height,
        "app_count": info.app_count,
    }))
}

pub async fn get_window_state(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let pid = require_u64(args, "pid")? as u32;
    let window_id = require_u64(args, "window_id")?
        as u32;
    let include_tree = args["include_tree"].as_bool().unwrap_or(true);
    let include_screenshot = args["include_screenshot"].as_bool().unwrap_or(false);

    let driver = find_driver(state, id).await?;
    let state = driver
        .get_window_state(pid, window_id, include_tree, include_screenshot)
        .await?;

    let mut result = json!({
        "window_id": state.window_id,
        "title": state.title,
        "app_name": state.app_name,
        "bounds": {
            "x": state.bounds.x,
            "y": state.bounds.y,
            "w": state.bounds.w,
            "h": state.bounds.h,
        },
    });

    if let Some(tree) = state.tree {
        let elements: Vec<Value> = tree
            .elements
            .iter()
            .map(|e| {
                let mut node = json!({
                    "index": e.index,
                    "role": e.role,
                    "name": e.name,
                    "actions": e.actions,
                    "depth": e.depth,
                });
                if let Some(p) = e.parent_index {
                    node["parent_index"] = json!(p);
                }
                if let Some(b) = &e.frame {
                    node["frame"] = json!({ "x": b.x, "y": b.y, "w": b.w, "h": b.h });
                }
                node
            })
            .collect();
        result["tree"] = json!({ "elements": elements });
    }

    if let Some(screenshot) = state.screenshot {
        result["screenshot"] = json!(encode_png(&screenshot.data));
    }

    Ok(result)
}

pub async fn get_desktop_overview(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let driver = find_driver(state, id).await?;
    let overview = driver.get_desktop_overview().await?;

    let processes: Vec<Value> = overview
        .processes
        .iter()
        .map(|p| json!({ "pid": p.pid, "name": p.name }))
        .collect();
    let windows: Vec<Value> = overview
        .windows
        .iter()
        .map(|w| {
            let mut node = json!({
                "window_id": w.id,
                "pid": w.pid,
                "title": w.title,
            });
            if let Some(b) = &w.bounds {
                node["bounds"] = json!({ "x": b.x, "y": b.y, "w": b.w, "h": b.h });
            }
            node
        })
        .collect();

    Ok(json!({ "processes": processes, "windows": windows }))
}

pub async fn set_value(state: &DaemonState, args: &Value) -> Result<Value, LxhError> {
    let id = require_str(args, "display_id")?;
    let pid = require_u64(args, "pid")? as u32;
    let index = require_u64(args, "index")?
        as usize;
    let value = require_str(args, "value")?;

    let driver = find_driver(state, id).await?;
    driver.set_value(pid, index, value).await?;
    Ok(json!({ "success": true }))
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "lxh_display_create",
            "description": "Create a new X11 display (backend: xvfb or xephyr). Set persistent to keep it after disconnect.",
            "inputSchema": { "type": "object", "properties": { "backend": { "type": "string", "enum": ["xvfb", "xephyr"] }, "persistent": { "type": "boolean" } } }
        }),
        json!({
            "name": "lxh_display_destroy",
            "description": "Destroy an X11 display",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_display_attach",
            "description": "Attach to an existing X11 display (e.g. :0)",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_display_detach",
            "description": "Detach from an existing X11 display without destroying it",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_app_launch",
            "description": "Launch an application on a display",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "command": { "type": "string" }, "args": { "type": "array", "items": { "type": "string" } } }, "required": ["display_id", "command"] }
        }),
        json!({
            "name": "lxh_app_terminate",
            "description": "Terminate an application by PID",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" } }, "required": ["display_id", "pid"] }
        }),
        json!({
            "name": "lxh_get_window_state",
            "description": "Get window state: metadata, optional AT-SPI tree, optional screenshot",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "window_id": { "type": "integer" }, "include_tree": { "type": "boolean" }, "include_screenshot": { "type": "boolean" } }, "required": ["display_id", "pid", "window_id"] }
        }),
        json!({
            "name": "lxh_input_click",
            "description": "Click at screen coordinates. Supports left/right/middle buttons and multiple clicks.",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" }, "button": { "type": "string", "enum": ["left", "right", "middle"] }, "count": { "type": "integer" } }, "required": ["display_id", "x", "y"] }
        }),
        json!({
            "name": "lxh_input_move",
            "description": "Move the mouse cursor",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" } }, "required": ["display_id", "x", "y"] }
        }),
        json!({
            "name": "lxh_input_type",
            "description": "Type text",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "text": { "type": "string" } }, "required": ["display_id", "text"] }
        }),
        json!({
            "name": "lxh_input_key",
            "description": "Press a key or key combination",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "key": { "type": "string" }, "modifiers": { "type": "array", "items": { "type": "string" } } }, "required": ["display_id", "key"] }
        }),
        json!({
            "name": "lxh_input_scroll",
            "description": "Scroll by a delta",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "dx": { "type": "integer" }, "dy": { "type": "integer" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_input_drag",
            "description": "Drag from (x1, y1) to (x2, y2)",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x1": { "type": "integer" }, "y1": { "type": "integer" }, "x2": { "type": "integer" }, "y2": { "type": "integer" } }, "required": ["display_id", "x1", "y1", "x2", "y2"] }
        }),
        json!({
            "name": "lxh_input_get_cursor_position",
            "description": "Get the current mouse cursor position",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_capture_screenshot",
            "description": "Take a screenshot",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_capture_window",
            "description": "Take a screenshot of a specific window",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" } }, "required": ["display_id", "window_id"] }
        }),
        json!({
            "name": "lxh_get_desktop_overview",
            "description": "Return desktop overview: running processes and visible windows",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_set_value",
            "description": "Set the value of an AT-SPI editable element",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "index": { "type": "integer" }, "value": { "type": "string" } }, "required": ["display_id", "pid", "index", "value"] }
        }),
        json!({
            "name": "lxh_click_element",
            "description": "Click an AT-SPI element by pid and index",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "index": { "type": "integer" }, "button": { "type": "string", "enum": ["left", "right", "middle"] } }, "required": ["display_id", "pid", "index"] }
        }),
        json!({
            "name": "lxh_wait",
            "description": "Wait for a number of milliseconds",
            "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer", "minimum": 0 } }, "required": ["ms"] }
        }),
        json!({
            "name": "lxh_window_focus",
            "description": "Focus a window by id",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" } }, "required": ["display_id", "window_id"] }
        }),
        json!({
            "name": "lxh_window_set_frame",
            "description": "Set a window's position and size",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" }, "x": { "type": "integer" }, "y": { "type": "integer" }, "width": { "type": "integer" }, "height": { "type": "integer" } }, "required": ["display_id", "window_id", "x", "y", "width", "height"] }
        }),
        json!({
            "name": "lxh_window_close",
            "description": "Close a window by id",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" } }, "required": ["display_id", "window_id"] }
        }),
        json!({
            "name": "lxh_clipboard_get",
            "description": "Get text from the clipboard",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxh_clipboard_set",
            "description": "Set text on the clipboard",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "text": { "type": "string" } }, "required": ["display_id", "text"] }
        }),
        json!({
            "name": "lxh_display_info",
            "description": "Get display metadata",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
    ]
}

async fn find_display(state: &DaemonState, id: &str) -> Result<Arc<Mutex<Display>>, LxhError> {
    state
        .displays
        .read()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| LxhError::DisplayNotFound(id.into()))
}

async fn find_driver(state: &DaemonState, id: &str) -> Result<Arc<dyn Driver>, LxhError> {
    state
        .drivers
        .read()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| LxhError::DisplayNotFound(id.into()))
}

fn encode_png(data: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
}
