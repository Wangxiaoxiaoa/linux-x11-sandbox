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

    fn keycode(&self, key: &str) -> u32 {
        let name = CString::new(key).unwrap();
        unsafe {
            let keysym = xlib::XStringToKeysym(name.as_ptr());
            xlib::XKeysymToKeycode(self.display, keysym) as u32
        }
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

    async fn scroll(&self, _dx: i32, _dy: i32) -> Result<(), LxsError> {
        Err(LxsError::NotImplemented)
    }

    async fn type_text(&self, text: &str) -> Result<(), LxsError> {
        unsafe {
            for ch in text.chars() {
                let keycode = self.keycode(&ch.to_string());
                xtest::XTestFakeKeyEvent(self.display, keycode, xlib::True, xlib::CurrentTime);
                xtest::XTestFakeKeyEvent(self.display, keycode, xlib::False, xlib::CurrentTime);
            }
            xlib::XFlush(self.display);
        }
        Ok(())
    }

    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError> {
        unsafe {
            let codes: Vec<u32> = modifiers
                .iter()
                .map(|m| self.keycode(m))
                .chain(std::iter::once(self.keycode(key)))
                .collect();

            for code in &codes {
                xtest::XTestFakeKeyEvent(self.display, *code, xlib::True, xlib::CurrentTime);
            }
            for code in codes.iter().rev() {
                xtest::XTestFakeKeyEvent(self.display, *code, xlib::False, xlib::CurrentTime);
            }

            xlib::XFlush(self.display);
        }
        Ok(())
    }
}
