use async_trait::async_trait;
use image::{ImageEncoder, RgbImage};
use lxs_core::{A11yBackend, Bounds, CaptureBackend, LxsError, Rect, Screenshot, WindowState};
use std::ffi::CString;
use x11::xlib;

pub struct X11Capture {
    display: *mut xlib::Display,
}

unsafe impl Send for X11Capture {}
unsafe impl Sync for X11Capture {}

impl X11Capture {
    pub fn new(display: &str) -> Self {
        let name = CString::new(display).unwrap();
        let dpy = unsafe { xlib::XOpenDisplay(name.as_ptr()) };
        assert!(!dpy.is_null());
        Self { display: dpy }
    }
}

impl Drop for X11Capture {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}

#[async_trait]
impl CaptureBackend for X11Capture {
    async fn screenshot(&self) -> Result<Screenshot, LxsError> {
        unsafe {
            let screen = xlib::XDefaultScreen(self.display);
            let root = xlib::XRootWindow(self.display, screen);
            let width = xlib::XDisplayWidth(self.display, screen);
            let height = xlib::XDisplayHeight(self.display, screen);

            let image = xlib::XGetImage(
                self.display,
                root,
                0,
                0,
                width as u32,
                height as u32,
                xlib::XAllPlanes(),
                xlib::ZPixmap,
            );
            assert!(!image.is_null());

            let bytes_per_line = (*image).bytes_per_line;
            let bits_per_pixel = (*image).bits_per_pixel;
            let data = (*image).data as *const u8;
            let mut buf = vec![0u8; (width * height * 3) as usize];

            for y in 0..height {
                for x in 0..width {
                    let offset = (y * bytes_per_line + x * (bits_per_pixel / 8)) as isize;
                    let pixel = *(data.offset(offset) as *const u32);
                    let idx = ((y * width + x) * 3) as usize;
                    buf[idx] = ((pixel >> 16) & 0xff) as u8;
                    buf[idx + 1] = ((pixel >> 8) & 0xff) as u8;
                    buf[idx + 2] = (pixel & 0xff) as u8;
                }
            }

            xlib::XDestroyImage(image);

            let img = RgbImage::from_raw(width as u32, height as u32, buf).unwrap();
            let mut png = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png)
                .write_image(&img, width as u32, height as u32, image::ExtendedColorType::Rgb8)
                .unwrap();

            Ok(Screenshot { data: png })
        }
    }

    async fn screenshot_region(&self, _region: Rect) -> Result<Screenshot, LxsError> {
        Err(LxsError::NotImplemented)
    }
}

pub struct AtspiA11y;

impl AtspiA11y {
    pub fn new(_display: &str) -> Self {
        Self
    }
}

#[async_trait]
impl A11yBackend for AtspiA11y {
    async fn window_state(&self) -> Result<WindowState, LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn accessibility_tree(&self, _pid: Option<u32>) -> Result<lxs_core::AccessibilityTree, LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn element_bounds(&self, _pid: u32, _index: usize) -> Result<Bounds, LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn perform_action(&self, _pid: u32, _index: usize, _action: &str) -> Result<(), LxsError> {
        Err(LxsError::NotImplemented)
    }
}
