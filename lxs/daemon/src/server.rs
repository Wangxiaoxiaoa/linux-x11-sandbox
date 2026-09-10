use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lxs_core::LxsError;
use lxs_runtime::Runtime;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;

use crate::handlers::{self, ClientSession, DaemonState};
use crate::protocol::{Request, Response};

pub struct DaemonServer {
    state: Arc<DaemonState>,
    socket_path: PathBuf,
}

impl DaemonServer {
    pub fn new(runtime: Arc<Runtime>, socket_path: PathBuf) -> Self {
        Self {
            state: Arc::new(DaemonState {
                runtime,
                displays: Arc::new(RwLock::new(HashMap::new())),
            }),
            socket_path,
        }
    }

    pub async fn run(&self) -> Result<(), LxsError> {
        let _ = std::fs::remove_file(&self.socket_path);
        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| LxsError::InvalidArgument(format!("cannot bind socket: {e}")))?;

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| LxsError::InvalidArgument(format!("accept failed: {e}")))?;
            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                let mut session = ClientSession::new();
                let state2 = Arc::clone(&state);
                if let Err(e) = handle_client(state, &mut session, stream).await {
                    eprintln!("client handler error: {e}");
                }
                cleanup_session(state2, &mut session).await;
            });
        }
    }
}

async fn handle_client(
    state: Arc<DaemonState>,
    session: &mut ClientSession,
    stream: UnixStream,
) -> Result<(), LxsError> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| LxsError::InvalidArgument(format!("read failed: {e}")))?;
        if n == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }

        let req: Request = serde_json::from_str(&line)
            .map_err(|e| LxsError::InvalidArgument(format!("invalid json: {e}")))?;

        let resp = dispatch(&state, session, req).await;
        let msg = serde_json::to_string(&resp)
            .map_err(|e| LxsError::InvalidArgument(format!("serialize failed: {e}")))?;
        write_half
            .write_all(msg.as_bytes())
            .await
            .map_err(|e| LxsError::InvalidArgument(format!("write failed: {e}")))?;
        write_half
            .write_all(b"\n")
            .await
            .map_err(|e| LxsError::InvalidArgument(format!("write failed: {e}")))?;
    }

    Ok(())
}

async fn dispatch(state: &DaemonState, session: &mut ClientSession, req: Request) -> Response {
    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "linux-x11-sandbox", "version": "0.1.0" }
        })),
        "tools/list" => Ok(json!({ "tools": handlers::tool_definitions() })),
        "tools/call" => dispatch_tool_call(state, session, &req.params).await,
        _ => Err(LxsError::InvalidArgument(format!(
            "unknown method: {}",
            req.method
        ))),
    };

    match result {
        Ok(value) => Response::result(req.id, value),
        Err(e) => Response::error(req.id, -32603, &e.to_string()),
    }
}

async fn dispatch_tool_call(
    state: &DaemonState,
    session: &mut ClientSession,
    params: &Value,
) -> Result<Value, LxsError> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| LxsError::InvalidArgument("tool name required".into()))?;
    let args = &params["arguments"];
    match name {
        "lxs_display_create" => handlers::create_display(state, session, args).await,
        "lxs_display_destroy" => handlers::destroy_display(state, session, args).await,
        "lxs_display_attach" => handlers::attach_display(state, args).await,
        "lxs_display_detach" => handlers::detach_display(state, args).await,
        "lxs_app_launch" => handlers::app_launch(state, args).await,
        "lxs_app_terminate" => handlers::app_terminate(state, args).await,
        "lxs_input_click" => handlers::click(state, args).await,
        "lxs_input_move" => handlers::move_mouse(state, args).await,
        "lxs_input_type" => handlers::input_type(state, args).await,
        "lxs_input_key" => handlers::input_key(state, args).await,
        "lxs_input_scroll" => handlers::scroll(state, args).await,
        "lxs_input_drag" => handlers::drag(state, args).await,
        "lxs_input_get_cursor_position" => handlers::get_cursor_position(state, args).await,
        "lxs_capture_screenshot" => handlers::screenshot(state, args).await,
        "lxs_capture_window" => handlers::screenshot_window(state, args).await,
        "lxs_window_focus" => handlers::window_focus(state, args).await,
        "lxs_window_set_frame" => handlers::window_set_frame(state, args).await,
        "lxs_window_close" => handlers::window_close(state, args).await,
        "lxs_click_element" => handlers::click_element(state, args).await,
        "lxs_wait" => handlers::wait(state, args).await,
        "lxs_clipboard_get" => handlers::clipboard_get(state, args).await,
        "lxs_clipboard_set" => handlers::clipboard_set(state, args).await,
        "lxs_display_info" => handlers::display_info(state, args).await,
        "lxs_get_window_state" => handlers::get_window_state(state, args).await,
        "lxs_get_desktop_overview" => handlers::get_desktop_overview(state, args).await,
        "lxs_set_value" => handlers::set_value(state, args).await,
        _ => Err(LxsError::InvalidArgument(format!("unknown tool: {name}"))),
    }
}

async fn cleanup_session(state: Arc<DaemonState>, session: &mut ClientSession) {
    let mut displays = state.displays.write().await;
    for id in session.owned_displays.drain() {
        if let Some(display) = displays.remove(&id) {
            let _ = display.lock().await.destroy().await;
        }
    }
}
