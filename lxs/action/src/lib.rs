use std::ffi::CString;

use async_trait::async_trait;
use lxs_core::{InputBackend, LxsError, MouseButton};
use tokio::task;
use x11::xlib;
use x11::xtest;

mod x11_util;

pub struct XtestInput {
    display: x11_util::DisplayHandle,
}

impl XtestInput {
    pub fn new(display: &str) -> Result<Self, LxsError> {
        Ok(Self {
            display: x11_util::open_display(display)?,
        })
    }

    fn keycode(display: *mut xlib::Display, key: &str) -> u32 {
        let name = CString::new(key).unwrap();
        unsafe {
            let keysym = xlib::XStringToKeysym(name.as_ptr());
            xlib::XKeysymToKeycode(display, keysym) as u32
        }
    }
}

#[async_trait]
impl InputBackend for XtestInput {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let dpy = display.lock().unwrap();
            unsafe {
                let root = xlib::XDefaultRootWindow(dpy.ptr());
                xlib::XWarpPointer(dpy.ptr(), 0, root, 0, 0, 0, 0, x, y);

                let button = match button {
                    MouseButton::Left => xlib::Button1,
                    MouseButton::Middle => xlib::Button2,
                    MouseButton::Right => xlib::Button3,
                };

                for _ in 0..count {
                    xtest::XTestFakeButtonEvent(dpy.ptr(), button, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeButtonEvent(dpy.ptr(), button, xlib::False, xlib::CurrentTime);
                }

                xlib::XFlush(dpy.ptr());
            }
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let dpy = display.lock().unwrap();
            unsafe {
                let root = xlib::XDefaultRootWindow(dpy.ptr());
                xlib::XWarpPointer(dpy.ptr(), 0, root, 0, 0, 0, 0, x, y);
                xlib::XFlush(dpy.ptr());
            }
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let dpy = display.lock().unwrap();
            unsafe {
                let click = |button: u32| {
                    xtest::XTestFakeButtonEvent(dpy.ptr(), button, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeButtonEvent(dpy.ptr(), button, xlib::False, xlib::CurrentTime);
                };

                for _ in 0..dy.abs() {
                    click(if dy > 0 { xlib::Button5 } else { xlib::Button4 });
                }
                for _ in 0..dx.abs() {
                    click(if dx > 0 { 7 } else { 6 });
                }

                xlib::XFlush(dpy.ptr());
            }
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn type_text(&self, text: &str) -> Result<(), LxsError> {
        let display = self.display.clone();
        let text = text.to_string();
        task::spawn_blocking(move || {
            let dpy = display.lock().unwrap();
            unsafe {
                for ch in text.chars() {
                    let keycode = Self::keycode(dpy.ptr(), &ch.to_string());
                    xtest::XTestFakeKeyEvent(dpy.ptr(), keycode, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeKeyEvent(dpy.ptr(), keycode, xlib::False, xlib::CurrentTime);
                }
                xlib::XFlush(dpy.ptr());
            }
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError> {
        let display = self.display.clone();
        let key = key.to_string();
        let modifiers: Vec<String> = modifiers.iter().map(|s| s.to_string()).collect();
        task::spawn_blocking(move || {
            let dpy = display.lock().unwrap();
            unsafe {
                let codes: Vec<u32> = modifiers
                    .iter()
                    .map(|m| Self::keycode(dpy.ptr(), m))
                    .chain(std::iter::once(Self::keycode(dpy.ptr(), &key)))
                    .collect();

                for code in &codes {
                    xtest::XTestFakeKeyEvent(dpy.ptr(), *code, xlib::True, xlib::CurrentTime);
                }
                for code in codes.iter().rev() {
                    xtest::XTestFakeKeyEvent(dpy.ptr(), *code, xlib::False, xlib::CurrentTime);
                }

                xlib::XFlush(dpy.ptr());
            }
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }
}
