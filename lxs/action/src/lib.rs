use async_trait::async_trait;
use lxs_core::{InputBackend, LxsError, MouseButton};
use std::ffi::CString;
use std::sync::Once;
use tokio::task;
use x11::xlib;
use x11::xtest;

static INIT_THREADS: Once = Once::new();

fn xerr(msg: impl Into<String>) -> LxsError {
    LxsError::InvalidArgument(msg.into())
}

fn open_display(name: &str) -> Result<*mut xlib::Display, LxsError> {
    INIT_THREADS.call_once(|| unsafe {
        xlib::XInitThreads();
    });

    let cname = CString::new(name).map_err(|e| xerr(e.to_string()))?;
    let dpy = unsafe { xlib::XOpenDisplay(cname.as_ptr()) };
    if dpy.is_null() {
        return Err(xerr(format!("cannot open display {name}")));
    }
    Ok(dpy)
}

fn close_display(dpy: *mut xlib::Display) {
    if !dpy.is_null() {
        unsafe { xlib::XCloseDisplay(dpy) };
    }
}

fn keysym_for_name(name: &str) -> Option<u64> {
    x11_keysymdef::lookup_by_name(name).map(|r| r.keysym as u64)
}

fn keycode_for_keysym(dpy: *mut xlib::Display, keysym: u64) -> Option<u8> {
    let code = unsafe { xlib::XKeysymToKeycode(dpy, keysym as xlib::KeySym) };
    if code == 0 {
        None
    } else {
        Some(code)
    }
}

fn keycode_for_name(dpy: *mut xlib::Display, name: &str) -> Result<u8, LxsError> {
    let keysym = keysym_for_name(name).ok_or_else(|| xerr(format!("unknown key {name}")))?;
    keycode_for_keysym(dpy, keysym).ok_or_else(|| xerr(format!("no keycode for {name}")))
}

pub struct XtestInput {
    display: String,
}

impl XtestInput {
    pub fn new(display: &str) -> Result<Self, LxsError> {
        let _ = open_display(display)?;
        Ok(Self {
            display: display.to_string(),
        })
    }
}

#[async_trait]
impl InputBackend for XtestInput {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let dpy = open_display(&display)?;
            unsafe {
                let root = xlib::XDefaultRootWindow(dpy);
                xlib::XWarpPointer(dpy, 0, root, 0, 0, 0, 0, x, y);

                let button = match button {
                    MouseButton::Left => xlib::Button1,
                    MouseButton::Middle => xlib::Button2,
                    MouseButton::Right => xlib::Button3,
                };

                for _ in 0..count {
                    xtest::XTestFakeButtonEvent(dpy, button, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeButtonEvent(dpy, button, xlib::False, xlib::CurrentTime);
                }

                xlib::XFlush(dpy);
            }
            close_display(dpy);
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let dpy = open_display(&display)?;
            unsafe {
                let root = xlib::XDefaultRootWindow(dpy);
                xlib::XWarpPointer(dpy, 0, root, 0, 0, 0, 0, x, y);
                xlib::XFlush(dpy);
            }
            close_display(dpy);
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let dpy = open_display(&display)?;
            unsafe {
                for _ in 0..dy.abs() {
                    let button = if dy > 0 { xlib::Button5 } else { xlib::Button4 };
                    xtest::XTestFakeButtonEvent(dpy, button, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeButtonEvent(dpy, button, xlib::False, xlib::CurrentTime);
                }
                for _ in 0..dx.abs() {
                    let button = if dx > 0 { 7 } else { 6 };
                    xtest::XTestFakeButtonEvent(dpy, button, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeButtonEvent(dpy, button, xlib::False, xlib::CurrentTime);
                }
                xlib::XFlush(dpy);
            }
            close_display(dpy);
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn type_text(&self, text: &str) -> Result<(), LxsError> {
        let display = self.display.clone();
        let text = text.to_string();
        task::spawn_blocking(move || {
            let dpy = open_display(&display)?;
            unsafe {
                for ch in text.chars() {
                    let keysym = x11_keysymdef::lookup_by_codepoint(ch)
                        .map(|r| r.keysym as u64)
                        .ok_or_else(|| xerr(format!("unknown character {ch}")))?;
                    let keycode = keycode_for_keysym(dpy, keysym)
                        .ok_or_else(|| xerr(format!("no keycode for {ch}")))?
                        as u32;
                    xtest::XTestFakeKeyEvent(dpy, keycode, xlib::True, xlib::CurrentTime);
                    xtest::XTestFakeKeyEvent(dpy, keycode, xlib::False, xlib::CurrentTime);
                }
                xlib::XFlush(dpy);
            }
            close_display(dpy);
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
            let dpy = open_display(&display)?;
            unsafe {
                let mut codes = Vec::with_capacity(modifiers.len() + 1);
                for m in &modifiers {
                    codes.push(keycode_for_name(dpy, m)?);
                }
                codes.push(keycode_for_name(dpy, &key)?);

                for code in &codes {
                    xtest::XTestFakeKeyEvent(dpy, *code as u32, xlib::True, xlib::CurrentTime);
                }
                for code in codes.iter().rev() {
                    xtest::XTestFakeKeyEvent(dpy, *code as u32, xlib::False, xlib::CurrentTime);
                }
                xlib::XFlush(dpy);
            }
            close_display(dpy);
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysym_lookup() {
        assert!(keysym_for_name("Return").is_some());
        assert!(keysym_for_name("Shift_L").is_some());
        assert!(x11_keysymdef::lookup_by_codepoint('a').is_some());
    }

    #[test]
    fn keycode_for_keysym_finds_slot_zero() {
        let dpy = open_display(":0").expect("need local display");
        let code = keycode_for_keysym(dpy, keysym_for_name("a").unwrap()).unwrap();
        assert!(code > 0);
        close_display(dpy);
    }

    #[test]
    fn keycode_for_keysym_finds_shifted_slot() {
        let dpy = open_display(":0").expect("need local display");
        let code = keycode_for_keysym(dpy, keysym_for_name("A").unwrap()).unwrap();
        assert!(code > 0);
        close_display(dpy);
    }
}
