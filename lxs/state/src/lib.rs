use async_trait::async_trait;
use image::{ImageEncoder, RgbImage};
use lxs_core::{A11yBackend, Bounds, CaptureBackend, LxsError, Rect, Screenshot, WindowState};
use tokio::task;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::rust_connection::{ConnectError, RustConnection};

pub mod atspi;

fn xerr<E: std::fmt::Display>(e: E) -> LxsError {
    LxsError::InvalidArgument(e.to_string())
}

fn open_connection(display: &str) -> Result<(RustConnection, usize), LxsError> {
    RustConnection::connect(Some(display))
        .map_err(|e: ConnectError| LxsError::InvalidArgument(format!("cannot open display: {e}")))
}

fn window_atom(conn: &RustConnection) -> Result<u32, LxsError> {
    Ok(conn
        .intern_atom(false, b"WINDOW")
        .map_err(xerr)?
        .reply()
        .map_err(xerr)?
        .atom)
}

pub struct X11Capture {
    display: String,
}

impl X11Capture {
    pub fn new(display: &str) -> Result<Self, LxsError> {
        let _ = open_connection(display)?;
        Ok(Self {
            display: display.to_string(),
        })
    }
}

#[async_trait]
impl CaptureBackend for X11Capture {
    async fn screenshot(&self) -> Result<Screenshot, LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let (conn, screen) = open_connection(&display)?;
            let root = conn.setup().roots[screen].root;
            let w = conn.setup().roots[screen].width_in_pixels;
            let h = conn.setup().roots[screen].height_in_pixels;
            capture_rect(&conn, root, 0, 0, w, h)
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let (conn, screen) = open_connection(&display)?;
            let root = conn.setup().roots[screen].root;
            capture_rect(
                &conn,
                root,
                region.x as i16,
                region.y as i16,
                region.w as u16,
                region.h as u16,
            )
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }
}

fn capture_rect(
    conn: &RustConnection,
    root: u32,
    x: i16,
    y: i16,
    w: u16,
    h: u16,
) -> Result<Screenshot, LxsError> {
    let reply = conn
        .get_image(
            x11rb::protocol::xproto::ImageFormat::Z_PIXMAP,
            root,
            x,
            y,
            w,
            h,
            u32::MAX,
        )
        .map_err(xerr)?
        .reply()
        .map_err(xerr)?;

    let data = reply.data;
    let stride = data.len() / h as usize;
    let mut buf = vec![0u8; (w as u32 * h as u32 * 3) as usize];

    for row in 0..h as usize {
        for col in 0..w as usize {
            let offset = row * stride + col * 4;
            let pixel = u32::from_ne_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let idx = (row * w as usize + col) * 3;
            buf[idx] = ((pixel >> 16) & 0xff) as u8;
            buf[idx + 1] = ((pixel >> 8) & 0xff) as u8;
            buf[idx + 2] = (pixel & 0xff) as u8;
        }
    }

    let img = RgbImage::from_raw(w as u32, h as u32, buf)
        .ok_or_else(|| LxsError::InvalidArgument("failed to create image buffer".into()))?;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&img, w as u32, h as u32, image::ExtendedColorType::Rgb8)
        .map_err(|e| LxsError::InvalidArgument(e.to_string()))?;

    Ok(Screenshot { data: png })
}

pub struct AtspiA11y {
    display: String,
    elements: std::sync::Mutex<Option<Vec<atspi::Element>>>,
}

impl AtspiA11y {
    pub fn new(display: &str) -> Result<Self, LxsError> {
        let _ = open_connection(display)?;
        Ok(Self {
            display: display.to_string(),
            elements: std::sync::Mutex::new(None),
        })
    }
}

#[async_trait]
impl A11yBackend for AtspiA11y {
    async fn window_state(&self) -> Result<WindowState, LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let (conn, screen) = open_connection(&display)?;
            let root = conn.setup().roots[screen].root;

            let net_active = conn
                .intern_atom(false, b"_NET_ACTIVE_WINDOW")
                .map_err(xerr)?
                .reply()
                .map_err(xerr)?
                .atom;
            let net_name = conn
                .intern_atom(false, b"_NET_WM_NAME")
                .map_err(xerr)?
                .reply()
                .map_err(xerr)?
                .atom;
            let utf8 = conn
                .intern_atom(false, b"UTF8_STRING")
                .map_err(xerr)?
                .reply()
                .map_err(xerr)?
                .atom;

            let active_reply = conn
                .get_property(false, root, net_active, window_atom(&conn)?, 0, 1)
                .map_err(xerr)?
                .reply()
                .map_err(xerr)?;

            let mut title = None;
            if active_reply.format == 32 && active_reply.value.len() >= 4 {
                let window = u32::from_ne_bytes([
                    active_reply.value[0],
                    active_reply.value[1],
                    active_reply.value[2],
                    active_reply.value[3],
                ]);

                let name_reply = conn
                    .get_property(false, window, net_name, utf8, 0, 1024)
                    .map_err(xerr)?
                    .reply()
                    .map_err(xerr)?;
                if !name_reply.value.is_empty() {
                    title = String::from_utf8(name_reply.value).ok();
                }
            }

            Ok(WindowState { title })
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn accessibility_tree(
        &self,
        pid: Option<u32>,
    ) -> Result<lxs_core::AccessibilityTree, LxsError> {
        let pid = pid.ok_or_else(|| LxsError::InvalidArgument("pid required".into()))?;
        let walked = atspi::walk_tree(pid).await?;
        let tree = atspi::accessibility_tree(&walked);
        *self.elements.lock().unwrap() = Some(walked);
        Ok(tree)
    }

    async fn element_bounds(&self, _pid: u32, index: usize) -> Result<Bounds, LxsError> {
        let elements = self.elements.lock().unwrap();
        let list = elements.as_ref().ok_or(LxsError::NotImplemented)?;
        let element = list.get(index).ok_or(LxsError::NotImplemented)?;
        element.bounds.clone().ok_or(LxsError::NotImplemented)
    }

    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError> {
        atspi::perform_action(pid, index, action).await
    }
}
