use async_trait::async_trait;
use lxs_action::XtestInput;
use lxs_core::{
    A11yBackend, Bounds, CaptureBackend, Driver, InputBackend, LxsError, MouseButton, Rect,
    Screenshot, WindowState,
};
use lxs_state::{AtspiA11y, X11Capture};

pub struct NativeDriver {
    input: XtestInput,
    capture: X11Capture,
    a11y: AtspiA11y,
}

impl NativeDriver {
    pub fn new(display: &str) -> Self {
        Self {
            input: XtestInput::new(display),
            capture: X11Capture::new(display),
            a11y: AtspiA11y::new(display),
        }
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

    async fn scroll(&self, _dx: i32, _dy: i32) -> Result<(), LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn type_text(&self, _text: &str) -> Result<(), LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn key(&self, _key: &str, _modifiers: &[&str]) -> Result<(), LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn screenshot(&self) -> Result<Screenshot, LxsError> {
        self.capture.screenshot().await
    }

    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError> {
        self.capture.screenshot_region(region).await
    }

    async fn window_state(&self) -> Result<WindowState, LxsError> {
        self.a11y.window_state().await
    }

    async fn accessibility_tree(&self, pid: Option<u32>) -> Result<lxs_core::AccessibilityTree, LxsError> {
        self.a11y.accessibility_tree(pid).await
    }

    async fn element_bounds(&self, pid: u32, index: usize) -> Result<Bounds, LxsError> {
        self.a11y.element_bounds(pid, index).await
    }

    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError> {
        self.a11y.perform_action(pid, index, action).await
    }
}
