use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lxs_core::{Driver, LxsError, MouseButton};
use lxs_runtime::{Backend, Display, DisplayConfig, Runtime};
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};

pub struct DaemonState {
    pub runtime: Arc<Runtime>,
    pub displays: Arc<RwLock<HashMap<String, Arc<Mutex<Display>>>>>,
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

pub async fn create_display(
    state: &DaemonState,
    session: &mut ClientSession,
    args: &Value,
) -> Result<Value, LxsError> {
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

    let display = state.runtime.create_display(config).await?;
    let id = display.id().to_string();
    let display_str = display.display().to_string();
    state
        .displays
        .write()
        .await
        .insert(id.clone(), Arc::new(Mutex::new(display)));
    if !args["persistent"].as_bool().unwrap_or(false) {
        session.owned_displays.insert(id.clone());
    }

    Ok(json!({
        "display_id": id,
        "display": display_str
    }))
}

pub async fn attach_display(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let display_id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;

    let display = state.runtime.attach_display(display_id).await?;
    let id = display.id().to_string();
    let display_str = display.display().to_string();
    state
        .displays
        .write()
        .await
        .insert(id.clone(), Arc::new(Mutex::new(display)));

    Ok(json!({
        "display_id": id,
        "display": display_str
    }))
}

pub async fn destroy_display(
    state: &DaemonState,
    session: &mut ClientSession,
    args: &Value,
) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;

    let display = find_display(state, id).await?;
    display.lock().await.destroy().await?;

    state.displays.write().await.remove(id);
    session.owned_displays.remove(id);

    Ok(json!({ "success": true }))
}

pub async fn detach_display(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;

    let display = find_display(state, id).await?;
    let mut d = display.lock().await;
    if !d.is_external() {
        return Err(LxsError::InvalidArgument(
            "cannot detach a sandbox display; use lxs_display_destroy".into(),
        ));
    }
    d.detach().await?;
    drop(d);

    state.displays.write().await.remove(id);

    Ok(json!({ "success": true }))
}

pub async fn app_launch(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
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

    let display = find_display(state, id).await?;
    let pid = display.lock().await.launch_app(command, &arg_refs).await?;
    Ok(json!({ "pid": pid }))
}

pub async fn app_terminate(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let pid = args["pid"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))? as u32;

    let display = find_display(state, id).await?;
    display.lock().await.terminate_app(pid).await?;
    Ok(json!({ "success": true }))
}

pub async fn click(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
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
        "right" => MouseButton::Right,
        "middle" => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    let count = args["count"].as_u64().unwrap_or(1) as u32;

    let driver = find_driver(state, id).await?;
    driver.click(x, y, button, count).await?;
    Ok(json!({ "success": true }))
}

pub async fn move_mouse(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let x = args["x"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
    let y = args["y"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;

    let driver = find_driver(state, id).await?;
    driver.move_mouse(x, y).await?;
    Ok(json!({ "success": true }))
}

pub async fn input_type(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let text = args["text"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("text required".into()))?;

    let driver = find_driver(state, id).await?;
    driver.type_text(text).await?;
    Ok(json!({ "success": true }))
}

pub async fn input_key(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
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

    let driver = find_driver(state, id).await?;
    driver.key(key, &mod_refs).await?;
    Ok(json!({ "success": true }))
}

pub async fn scroll(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let dx = args["dx"].as_i64().unwrap_or(0) as i32;
    let dy = args["dy"].as_i64().unwrap_or(0) as i32;

    let driver = find_driver(state, id).await?;
    driver.scroll(dx, dy).await?;
    Ok(json!({ "success": true }))
}

pub async fn drag(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let x1 = args["x1"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("x1 required".into()))? as i32;
    let y1 = args["y1"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("y1 required".into()))? as i32;
    let x2 = args["x2"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("x2 required".into()))? as i32;
    let y2 = args["y2"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("y2 required".into()))? as i32;

    let driver = find_driver(state, id).await?;
    driver.drag(x1, y1, x2, y2).await?;
    Ok(json!({ "success": true }))
}

pub async fn get_cursor_position(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let driver = find_driver(state, id).await?;
    let (x, y) = driver.get_cursor_position().await?;
    Ok(json!({ "x": x, "y": y }))
}

pub async fn screenshot(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let driver = find_driver(state, id).await?;
    let shot = driver.screenshot().await?;
    Ok(json!({
        "mimeType": "image/png",
        "data": encode_png(&shot.data)
    }))
}

pub async fn screenshot_window(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let window_id = args["window_id"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("window_id required".into()))?
        as u32;

    let driver = find_driver(state, id).await?;
    let shot = driver.screenshot_window(window_id).await?;
    Ok(json!({
        "mimeType": "image/png",
        "data": encode_png(&shot.data)
    }))
}

pub async fn window_focus(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let window_id = args["window_id"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("window_id required".into()))?
        as u32;

    let driver = find_driver(state, id).await?;
    driver.focus_window(window_id).await?;
    Ok(json!({ "success": true }))
}

pub async fn window_set_frame(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let window_id = args["window_id"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("window_id required".into()))?
        as u32;
    let x = args["x"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("x required".into()))? as i32;
    let y = args["y"]
        .as_i64()
        .ok_or_else(|| LxsError::InvalidArgument("y required".into()))? as i32;
    let width = args["width"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("width required".into()))? as u32;
    let height = args["height"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("height required".into()))?
        as u32;

    let driver = find_driver(state, id).await?;
    driver
        .set_window_frame(window_id, x, y, width, height)
        .await?;
    Ok(json!({ "success": true }))
}

pub async fn window_close(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let window_id = args["window_id"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("window_id required".into()))?
        as u32;

    let driver = find_driver(state, id).await?;
    driver.close_window(window_id).await?;
    Ok(json!({ "success": true }))
}

pub async fn click_element(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let pid = args["pid"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))? as u32;
    let index = args["index"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("index required".into()))?
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

pub async fn wait(_state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let ms = args["ms"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("ms required".into()))?;
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    Ok(json!({ "success": true }))
}

pub async fn clipboard_get(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let driver = find_driver(state, id).await?;
    let text = driver.clipboard_get().await?;
    Ok(json!({ "text": text }))
}

pub async fn clipboard_set(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let text = args["text"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("text required".into()))?;
    let driver = find_driver(state, id).await?;
    driver.clipboard_set(text).await?;
    Ok(json!({ "success": true }))
}

pub async fn display_info(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let display = find_display(state, id).await?;
    let info = display.lock().await.info();
    Ok(json!({
        "display": info.display,
        "width": info.width,
        "height": info.height,
        "app_count": info.app_count,
    }))
}

pub async fn get_window_state(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let pid = args["pid"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))? as u32;
    let window_id = args["window_id"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("window_id required".into()))?
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

pub async fn get_desktop_overview(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
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

pub async fn set_value(state: &DaemonState, args: &Value) -> Result<Value, LxsError> {
    let id = args["display_id"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("display_id required".into()))?;
    let pid = args["pid"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("pid required".into()))? as u32;
    let index = args["index"]
        .as_u64()
        .ok_or_else(|| LxsError::InvalidArgument("index required".into()))?
        as usize;
    let value = args["value"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("value required".into()))?;

    let driver = find_driver(state, id).await?;
    driver.set_value(pid, index, value).await?;
    Ok(json!({ "success": true }))
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "lxs_display_create",
            "description": "Create a new X11 display (backend: xvfb or xephyr). Set persistent to keep it after disconnect.",
            "inputSchema": { "type": "object", "properties": { "backend": { "type": "string", "enum": ["xvfb", "xephyr"] }, "persistent": { "type": "boolean" } } }
        }),
        json!({
            "name": "lxs_display_destroy",
            "description": "Destroy an X11 display",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_display_attach",
            "description": "Attach to an existing X11 display (e.g. :0)",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_display_detach",
            "description": "Detach from an existing X11 display without destroying it",
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
            "name": "lxs_get_window_state",
            "description": "Get window state: metadata, optional AT-SPI tree, optional screenshot",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "window_id": { "type": "integer" }, "include_tree": { "type": "boolean" }, "include_screenshot": { "type": "boolean" } }, "required": ["display_id", "pid", "window_id"] }
        }),
        json!({
            "name": "lxs_input_click",
            "description": "Click at screen coordinates. Supports left/right/middle buttons and multiple clicks.",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x": { "type": "integer" }, "y": { "type": "integer" }, "button": { "type": "string", "enum": ["left", "right", "middle"] }, "count": { "type": "integer" } }, "required": ["display_id", "x", "y"] }
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
            "name": "lxs_input_scroll",
            "description": "Scroll by a delta",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "dx": { "type": "integer" }, "dy": { "type": "integer" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_input_drag",
            "description": "Drag from (x1, y1) to (x2, y2)",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "x1": { "type": "integer" }, "y1": { "type": "integer" }, "x2": { "type": "integer" }, "y2": { "type": "integer" } }, "required": ["display_id", "x1", "y1", "x2", "y2"] }
        }),
        json!({
            "name": "lxs_input_get_cursor_position",
            "description": "Get the current mouse cursor position",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_capture_screenshot",
            "description": "Take a screenshot",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_capture_window",
            "description": "Take a screenshot of a specific window",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" } }, "required": ["display_id", "window_id"] }
        }),
        json!({
            "name": "lxs_get_desktop_overview",
            "description": "Return desktop overview: running processes and visible windows",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
        json!({
            "name": "lxs_set_value",
            "description": "Set the value of an AT-SPI editable element",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "index": { "type": "integer" }, "value": { "type": "string" } }, "required": ["display_id", "pid", "index", "value"] }
        }),
        json!({
            "name": "lxs_click_element",
            "description": "Click an AT-SPI element by pid and index",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "pid": { "type": "integer" }, "index": { "type": "integer" }, "button": { "type": "string", "enum": ["left", "right", "middle"] } }, "required": ["display_id", "pid", "index"] }
        }),
        json!({
            "name": "lxs_wait",
            "description": "Wait for a number of milliseconds",
            "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer", "minimum": 0 } }, "required": ["ms"] }
        }),
        json!({
            "name": "lxs_window_focus",
            "description": "Focus a window by id",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" } }, "required": ["display_id", "window_id"] }
        }),
        json!({
            "name": "lxs_window_set_frame",
            "description": "Set a window's position and size",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" }, "x": { "type": "integer" }, "y": { "type": "integer" }, "width": { "type": "integer" }, "height": { "type": "integer" } }, "required": ["display_id", "window_id", "x", "y", "width", "height"] }
        }),
        json!({
            "name": "lxs_window_close",
            "description": "Close a window by id",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" }, "window_id": { "type": "integer" } }, "required": ["display_id", "window_id"] }
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
            "name": "lxs_display_info",
            "description": "Get display metadata",
            "inputSchema": { "type": "object", "properties": { "display_id": { "type": "string" } }, "required": ["display_id"] }
        }),
    ]
}

async fn find_display(state: &DaemonState, id: &str) -> Result<Arc<Mutex<Display>>, LxsError> {
    state
        .displays
        .read()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| LxsError::DisplayNotFound(id.into()))
}

async fn find_driver(state: &DaemonState, id: &str) -> Result<Arc<dyn Driver>, LxsError> {
    let display = find_display(state, id).await?;
    let driver = display.lock().await.driver();
    Ok(driver)
}

fn encode_png(data: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
}
