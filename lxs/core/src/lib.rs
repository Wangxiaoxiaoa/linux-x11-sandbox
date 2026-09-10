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

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub struct Screenshot {
    pub data: Vec<u8>,
}

pub struct WindowState {
    pub title: Option<String>,
}

pub struct WindowInfo {
    pub id: u32,
    pub title: Option<String>,
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
    pub actions: Vec<String>,
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
    async fn drag(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        button: MouseButton,
    ) -> Result<(), LxsError>;
    async fn type_text(&self, text: &str) -> Result<(), LxsError>;
    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError>;
    async fn get_cursor_position(&self) -> Result<(i32, i32), LxsError>;
}

#[async_trait]
pub trait CaptureBackend: Send + Sync {
    async fn screenshot(&self) -> Result<Screenshot, LxsError>;
    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError>;
}

#[async_trait]
pub trait WindowBackend: Send + Sync {
    async fn focus_window(&self) -> Result<(), LxsError>;
    async fn raise_window(&self) -> Result<(), LxsError>;
    async fn resize_window(&self, width: u32, height: u32) -> Result<(), LxsError>;
    async fn move_window(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn list_windows(&self) -> Result<Vec<WindowInfo>, LxsError>;
}

#[async_trait]
pub trait ClipboardBackend: Send + Sync {
    async fn clipboard_get(&self) -> Result<String, LxsError>;
    async fn clipboard_set(&self, text: &str) -> Result<(), LxsError>;
}

#[async_trait]
pub trait A11yBackend: Send + Sync {
    async fn window_state(&self) -> Result<WindowState, LxsError>;
    async fn accessibility_tree(&self, pid: Option<u32>) -> Result<AccessibilityTree, LxsError>;
    async fn element_bounds(&self, pid: u32, index: usize) -> Result<Bounds, LxsError>;
    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError>;
}

#[async_trait]
pub trait Driver: Send + Sync {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError>;
    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError>;
    async fn drag(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        button: MouseButton,
    ) -> Result<(), LxsError>;
    async fn type_text(&self, text: &str) -> Result<(), LxsError>;
    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError>;
    async fn get_cursor_position(&self) -> Result<(i32, i32), LxsError>;

    async fn screenshot(&self) -> Result<Screenshot, LxsError>;
    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError>;

    async fn focus_window(&self) -> Result<(), LxsError>;
    async fn raise_window(&self) -> Result<(), LxsError>;
    async fn resize_window(&self, width: u32, height: u32) -> Result<(), LxsError>;
    async fn move_window(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn list_windows(&self) -> Result<Vec<WindowInfo>, LxsError>;

    async fn clipboard_get(&self) -> Result<String, LxsError>;
    async fn clipboard_set(&self, text: &str) -> Result<(), LxsError>;

    async fn window_state(&self) -> Result<WindowState, LxsError>;
    async fn accessibility_tree(&self, pid: Option<u32>) -> Result<AccessibilityTree, LxsError>;
    async fn element_bounds(&self, pid: u32, index: usize) -> Result<Bounds, LxsError>;
    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError>;
}
