use async_trait::async_trait;
use lxs_core::{InputBackend, LxsError, MouseButton};
use std::ffi::CString;
use x11::xlib;
use x11::xtest;

pub struct XtestInput {
    display: *mut xlib::Display,
}

unsafe impl Send for XtestInput {}
unsafe impl Sync for XtestInput {}

impl XtestInput {
    pub fn new(display: &str) -> Self {
        let name = CString::new(display).unwrap();
        let dpy = unsafe { xlib::XOpenDisplay(name.as_ptr()) };
        assert!(!dpy.is_null());
        Self { display: dpy }
    }
}

impl Drop for XtestInput {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.display);
        }
    }
}

#[async_trait]
impl InputBackend for XtestInput {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError> {
        unsafe {
            let root = xlib::XDefaultRootWindow(self.display);
            xlib::XWarpPointer(self.display, 0, root, 0, 0, 0, 0, x, y);

            let button = match button {
                MouseButton::Left => xlib::Button1,
                MouseButton::Middle => xlib::Button2,
                MouseButton::Right => xlib::Button3,
            };

            for _ in 0..count {
                xtest::XTestFakeButtonEvent(self.display, button, xlib::True, xlib::CurrentTime);
                xtest::XTestFakeButtonEvent(self.display, button, xlib::False, xlib::CurrentTime);
            }

            xlib::XFlush(self.display);
        }
        Ok(())
    }

    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError> {
        unsafe {
            let root = xlib::XDefaultRootWindow(self.display);
            xlib::XWarpPointer(self.display, 0, root, 0, 0, 0, 0, x, y);
            xlib::XFlush(self.display);
        }
        Ok(())
    }
}
