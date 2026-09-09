use async_trait::async_trait;
use image::{ImageEncoder, RgbImage};
use lxs_core::{A11yBackend, Bounds, CaptureBackend, LxsError, Rect, Screenshot, WindowState};
use std::ffi::{CStr, CString};
use x11::xlib;

pub mod atspi;

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

pub struct AtspiA11y {
    display: *mut xlib::Display,
    elements: std::sync::Mutex<Option<Vec<atspi::Element>>>,
}

unsafe impl Send for AtspiA11y {}
unsafe impl Sync for AtspiA11y {}

impl AtspiA11y {
    pub fn new(display: &str) -> Self {
        let name = CString::new(display).unwrap();
        let dpy = unsafe { xlib::XOpenDisplay(name.as_ptr()) };
        assert!(!dpy.is_null());
        Self {
            display: dpy,
            elements: std::sync::Mutex::new(None),
        }
    }
}

impl Drop for AtspiA11y {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}

#[async_trait]
impl A11yBackend for AtspiA11y {
    async fn window_state(&self) -> Result<WindowState, LxsError> {
        unsafe {
            let screen = xlib::XDefaultScreen(self.display);
            let root = xlib::XRootWindow(self.display, screen);
            let net_active = xlib::XInternAtom(self.display, c"_NET_ACTIVE_WINDOW".as_ptr(), xlib::False);

            let mut actual_type = 0;
            let mut actual_format = 0;
            let mut nitems = 0;
            let mut bytes_after = 0;
            let mut prop: *mut u8 = std::ptr::null_mut();

            xlib::XGetWindowProperty(
                self.display,
                root,
                net_active,
                0,
                1,
                xlib::False,
                xlib::XA_WINDOW,
                &mut actual_type,
                &mut actual_format,
                &mut nitems,
                &mut bytes_after,
                &mut prop,
            );

            let mut title = None;
            if !prop.is_null() && nitems > 0 {
                let window = *(prop as *const xlib::Window);
                xlib::XFree(prop as *mut _);

                let net_name = xlib::XInternAtom(self.display, c"_NET_WM_NAME".as_ptr(), xlib::False);
                let utf8 = xlib::XInternAtom(self.display, c"UTF8_STRING".as_ptr(), xlib::False);

                xlib::XGetWindowProperty(
                    self.display,
                    window,
                    net_name,
                    0,
                    1024,
                    xlib::False,
                    utf8,
                    &mut actual_type,
                    &mut actual_format,
                    &mut nitems,
                    &mut bytes_after,
                    &mut prop,
                );

                if !prop.is_null() && nitems > 0 {
                    title = CStr::from_ptr(prop as *const i8).to_str().ok().map(String::from);
                    xlib::XFree(prop as *mut _);
                }
            }

            Ok(WindowState { title })
        }
    }

    async fn accessibility_tree(&self, pid: Option<u32>) -> Result<lxs_core::AccessibilityTree, LxsError> {
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
