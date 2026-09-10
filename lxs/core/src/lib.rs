use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum LxsError {
    #[error("not implemented")]
    NotImplemented,
    #[error("process spawn failed: {0}")]
    ProcessSpawnFailed(String),
    #[error("process kill failed: {0}")]
    ProcessKillFailed(String),
    #[error("display not found: {0}")]
    DisplayNotFound(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub mod x11 {
    use super::LxsError;
    use x11rb::connection::Connection;
    use x11rb::rust_connection::{ConnectError, RustConnection};

    pub fn xerr<E: std::fmt::Display>(e: E) -> LxsError {
        LxsError::InvalidArgument(e.to_string())
    }

    pub fn open_connection(display: &str) -> Result<(RustConnection, usize), LxsError> {
        RustConnection::connect(Some(display)).map_err(|e: ConnectError| {
            LxsError::InvalidArgument(format!("cannot open display: {e}"))
        })
    }

    pub fn root_window(conn: &RustConnection, screen: usize) -> u32 {
        conn.setup().roots[screen].root
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub struct Screenshot {
    pub data: Vec<u8>,
}

pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
}

pub struct WindowEntry {
    pub id: u32,
    pub pid: Option<u32>,
    pub title: Option<String>,
    pub bounds: Option<Bounds>,
}

pub struct DesktopOverview {
    pub processes: Vec<ProcessEntry>,
    pub windows: Vec<WindowEntry>,
}

pub struct GetWindowStateResult {
    pub window_id: u32,
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bounds: Bounds,
    pub tree: Option<AccessibilityTree>,
    pub screenshot: Option<Screenshot>,
}

pub struct DisplayInfo {
    pub display: String,
    pub width: u32,
    pub height: u32,
    pub app_count: usize,
}

pub struct A11yElement {
    pub index: usize,
    pub role: String,
    pub name: Option<String>,
    pub frame: Option<Bounds>,
    pub actions: Vec<String>,
    pub parent_index: Option<usize>,
    pub depth: usize,
}

pub struct AccessibilityTree {
    pub elements: Vec<A11yElement>,
}

#[derive(Clone, Debug)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[async_trait]
pub trait InputBackend: Send + Sync {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError>;
    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError>;
    async fn drag(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<(), LxsError>;
    async fn type_text(&self, text: &str) -> Result<(), LxsError>;
    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError>;
    async fn get_cursor_position(&self) -> Result<(i32, i32), LxsError>;
}

#[async_trait]
pub trait CaptureBackend: Send + Sync {
    async fn screenshot(&self) -> Result<Screenshot, LxsError>;
    async fn screenshot_window(&self, window_id: u32) -> Result<Screenshot, LxsError>;
}

#[async_trait]
pub trait WindowBackend: Send + Sync {
    async fn focus_window(&self, window_id: u32) -> Result<(), LxsError>;
    async fn set_window_frame(
        &self,
        window_id: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(), LxsError>;
    async fn close_window(&self, window_id: u32) -> Result<(), LxsError>;
}

#[async_trait]
pub trait ClipboardBackend: Send + Sync {
    async fn clipboard_get(&self) -> Result<String, LxsError>;
    async fn clipboard_set(&self, text: &str) -> Result<(), LxsError>;
}

#[async_trait]
pub trait A11yBackend: Send + Sync {
    async fn get_window_state(
        &self,
        pid: u32,
        window_id: u32,
        include_tree: bool,
        include_screenshot: bool,
    ) -> Result<GetWindowStateResult, LxsError>;
    async fn get_desktop_overview(&self) -> Result<DesktopOverview, LxsError>;
    async fn set_value(&self, pid: u32, index: usize, value: &str) -> Result<(), LxsError>;
    async fn element_frame(&self, pid: u32, index: usize) -> Result<Bounds, LxsError>;
}

#[async_trait]
pub trait Driver: Send + Sync {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError>;
    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError>;
    async fn drag(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<(), LxsError>;
    async fn type_text(&self, text: &str) -> Result<(), LxsError>;
    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError>;

    async fn screenshot(&self) -> Result<Screenshot, LxsError>;
    async fn screenshot_window(&self, window_id: u32) -> Result<Screenshot, LxsError>;

    async fn focus_window(&self, window_id: u32) -> Result<(), LxsError>;
    async fn set_window_frame(
        &self,
        window_id: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(), LxsError>;
    async fn close_window(&self, window_id: u32) -> Result<(), LxsError>;

    async fn clipboard_get(&self) -> Result<String, LxsError>;
    async fn clipboard_set(&self, text: &str) -> Result<(), LxsError>;

    async fn get_cursor_position(&self) -> Result<(i32, i32), LxsError>;

    async fn get_window_state(
        &self,
        pid: u32,
        window_id: u32,
        include_tree: bool,
        include_screenshot: bool,
    ) -> Result<GetWindowStateResult, LxsError>;
    async fn get_desktop_overview(&self) -> Result<DesktopOverview, LxsError>;
    async fn set_value(&self, pid: u32, index: usize, value: &str) -> Result<(), LxsError>;
    async fn click_element(
        &self,
        pid: u32,
        index: usize,
        button: MouseButton,
    ) -> Result<(), LxsError>;
}
