use async_trait::async_trait;
use lxs_action::{ArboardClipboard, X11WindowManager, XtestInput};
use lxs_core::{
    A11yBackend, Bounds, CaptureBackend, ClipboardBackend, Driver, InputBackend, LxsError,
    MouseButton, Rect, Screenshot, WindowBackend, WindowState,
};
use lxs_state::{AtspiA11y, X11Capture};

pub struct NativeDriver {
    input: XtestInput,
    capture: X11Capture,
    a11y: AtspiA11y,
    window: X11WindowManager,
    clipboard: ArboardClipboard,
}

impl NativeDriver {
    pub fn new(display: &str) -> Result<Self, LxsError> {
        Ok(Self {
            input: XtestInput::new(display)?,
            capture: X11Capture::new(display)?,
            a11y: AtspiA11y::new(display)?,
            window: X11WindowManager::new(display)?,
            clipboard: ArboardClipboard::new()?,
        })
    }
}

#[async_trait]
impl Driver for NativeDriver {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError> {
        self.input.click(x, y, button, count).await
    }

    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError> {
        self.input.move_mouse(x, y).await
    }

    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError> {
        self.input.scroll(dx, dy).await
    }

    async fn drag(
        &self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        button: MouseButton,
    ) -> Result<(), LxsError> {
        self.input.drag(x1, y1, x2, y2, button).await
    }

    async fn type_text(&self, text: &str) -> Result<(), LxsError> {
        self.input.type_text(text).await
    }

    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError> {
        self.input.key(key, modifiers).await
    }

    async fn screenshot(&self) -> Result<Screenshot, LxsError> {
        self.capture.screenshot().await
    }

    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError> {
        self.capture.screenshot_region(region).await
    }

    async fn focus_window(&self) -> Result<(), LxsError> {
        self.window.focus_window().await
    }

    async fn raise_window(&self) -> Result<(), LxsError> {
        self.window.raise_window().await
    }

    async fn resize_window(&self, width: u32, height: u32) -> Result<(), LxsError> {
        self.window.resize_window(width, height).await
    }

    async fn move_window(&self, x: i32, y: i32) -> Result<(), LxsError> {
        self.window.move_window(x, y).await
    }

    async fn clipboard_get(&self) -> Result<String, LxsError> {
        self.clipboard.clipboard_get().await
    }

    async fn clipboard_set(&self, text: &str) -> Result<(), LxsError> {
        self.clipboard.clipboard_set(text).await
    }

    async fn window_state(&self) -> Result<WindowState, LxsError> {
        self.a11y.window_state().await
    }

    async fn accessibility_tree(
        &self,
        pid: Option<u32>,
    ) -> Result<lxs_core::AccessibilityTree, LxsError> {
        self.a11y.accessibility_tree(pid).await
    }

    async fn element_bounds(&self, pid: u32, index: usize) -> Result<Bounds, LxsError> {
        self.a11y.element_bounds(pid, index).await
    }

    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError> {
        self.a11y.perform_action(pid, index, action).await
    }
}
