use async_trait::async_trait;
use lxs_core::{InputBackend, LxsError, MouseButton};
use tokio::task;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt as _, BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::{ConnectError, RustConnection};

fn xerr<E: std::fmt::Display>(e: E) -> LxsError {
    LxsError::InvalidArgument(e.to_string())
}

fn open_connection(display: &str) -> Result<(RustConnection, usize), LxsError> {
    RustConnection::connect(Some(display))
        .map_err(|e: ConnectError| LxsError::InvalidArgument(format!("cannot open display: {e}")))
}

fn root_window(conn: &RustConnection, screen: usize) -> u32 {
    conn.setup().roots[screen].root
}

fn keysym_for_char(ch: char) -> Option<u32> {
    x11_keysymdef::lookup_by_codepoint(ch).map(|r| r.keysym)
}

fn keysym_for_name(name: &str) -> Option<u32> {
    x11_keysymdef::lookup_by_name(name).map(|r| r.keysym)
}

fn keycode_for_keysym(
    mapping: &x11rb::protocol::xproto::GetKeyboardMappingReply,
    keysym: u32,
) -> Option<(u8, bool)> {
    let per = mapping.keysyms_per_keycode as usize;
    if per == 0 {
        return None;
    }
    for (i, syms) in mapping.keysyms.chunks(per).enumerate() {
        if syms.first() == Some(&keysym) {
            return Some(((8 + i) as u8, false));
        }
        if per > 1 && syms.get(1) == Some(&keysym) {
            return Some(((8 + i) as u8, true));
        }
    }
    None
}

fn get_mapping(
    conn: &RustConnection,
) -> Result<x11rb::protocol::xproto::GetKeyboardMappingReply, LxsError> {
    conn.get_keyboard_mapping(8, 248)
        .map_err(xerr)?
        .reply()
        .map_err(xerr)
}

pub struct XtestInput {
    display: String,
}

impl XtestInput {
    pub fn new(display: &str) -> Result<Self, LxsError> {
        let _ = open_connection(display)?;
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
            let (conn, screen) = open_connection(&display)?;
            let root = root_window(&conn, screen);
            let x16 = x as i16;
            let y16 = y as i16;

            conn.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, x16, y16)
                .map_err(xerr)?
                .check()
                .map_err(xerr)?;

            let button = match button {
                MouseButton::Left => 1,
                MouseButton::Middle => 2,
                MouseButton::Right => 3,
            };

            for _ in 0..count {
                conn.xtest_fake_input(BUTTON_PRESS_EVENT, button, 0, root, x16, y16, 0)
                    .map_err(xerr)?
                    .check()
                    .map_err(xerr)?;
                conn.xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, root, x16, y16, 0)
                    .map_err(xerr)?
                    .check()
                    .map_err(xerr)?;
            }

            conn.flush().map_err(xerr)?;
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let (conn, screen) = open_connection(&display)?;
            let root = root_window(&conn, screen);
            conn.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, x as i16, y as i16)
                .map_err(xerr)?
                .check()
                .map_err(xerr)?;
            conn.flush().map_err(xerr)?;
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError> {
        let display = self.display.clone();
        task::spawn_blocking(move || {
            let (conn, screen) = open_connection(&display)?;
            let root = root_window(&conn, screen);

            let click = |button: u8| -> Result<(), LxsError> {
                conn.xtest_fake_input(BUTTON_PRESS_EVENT, button, 0, root, 0, 0, 0)
                    .map_err(xerr)?
                    .check()
                    .map_err(xerr)?;
                conn.xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, root, 0, 0, 0)
                    .map_err(xerr)?
                    .check()
                    .map_err(xerr)?;
                Ok(())
            };

            for _ in 0..dy.abs() {
                click(if dy > 0 { 5 } else { 4 })?;
            }
            for _ in 0..dx.abs() {
                click(if dx > 0 { 7 } else { 6 })?;
            }

            conn.flush().map_err(xerr)?;
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }

    async fn type_text(&self, text: &str) -> Result<(), LxsError> {
        let display = self.display.clone();
        let text = text.to_string();
        task::spawn_blocking(move || {
            let (conn, _) = open_connection(&display)?;
            let mapping = get_mapping(&conn)?;

            for ch in text.chars() {
                let keysym = keysym_for_char(ch).ok_or_else(|| {
                    LxsError::InvalidArgument(format!("unknown character: {}", ch))
                })?;
                let (keycode, needs_shift) =
                    keycode_for_keysym(&mapping, keysym).ok_or_else(|| {
                        LxsError::InvalidArgument(format!("no keycode for keysym 0x{:x}", keysym))
                    })?;

                if needs_shift {
                    let (shift_kc, _) = keycode_for_keysym(&mapping, 0xffe1)
                        .ok_or_else(|| LxsError::InvalidArgument("no Shift keycode".into()))?;
                    press(&conn, shift_kc, true)?;
                    press(&conn, keycode, true)?;
                    press(&conn, keycode, false)?;
                    press(&conn, shift_kc, false)?;
                } else {
                    press(&conn, keycode, true)?;
                    press(&conn, keycode, false)?;
                }
            }

            conn.flush().map_err(xerr)?;
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
            let (conn, _) = open_connection(&display)?;
            let mapping = get_mapping(&conn)?;

            let keysym = keysym_for_name(&key)
                .or_else(|| keysym_for_char(key.chars().next().unwrap_or('\0')))
                .ok_or_else(|| LxsError::InvalidArgument(format!("unknown key: {}", key)))?;
            let (keycode, _) = keycode_for_keysym(&mapping, keysym)
                .ok_or_else(|| LxsError::InvalidArgument(format!("no keycode for key: {}", key)))?;

            let mut codes = Vec::new();
            for m in &modifiers {
                let mk = keysym_for_name(m)
                    .ok_or_else(|| LxsError::InvalidArgument(format!("unknown modifier: {}", m)))?;
                let (mkc, _) = keycode_for_keysym(&mapping, mk).ok_or_else(|| {
                    LxsError::InvalidArgument(format!("no keycode for modifier: {}", m))
                })?;
                codes.push(mkc);
            }
            codes.push(keycode);

            for &code in &codes {
                press(&conn, code, true)?;
            }
            for &code in codes.iter().rev() {
                press(&conn, code, false)?;
            }

            conn.flush().map_err(xerr)?;
            Ok(())
        })
        .await
        .map_err(|e| LxsError::ProcessSpawnFailed(e.to_string()))?
    }
}

fn press(conn: &RustConnection, keycode: u8, down: bool) -> Result<(), LxsError> {
    let event = if down {
        KEY_PRESS_EVENT
    } else {
        KEY_RELEASE_EVENT
    };
    conn.xtest_fake_input(event, keycode, 0, x11rb::NONE, 0, 0, 0)
        .map_err(xerr)?
        .check()
        .map_err(xerr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysym_lookup() {
        assert!(keysym_for_name("Return").is_some());
        assert!(keysym_for_name("Shift_L").is_some());
        assert!(keysym_for_char('a').is_some());
    }

    #[test]
    fn keycode_for_keysym_finds_slot_zero() {
        let (conn, _) = open_connection(":0").expect("need local display");
        let mapping = get_mapping(&conn).expect("keyboard mapping");
        let keysym = keysym_for_name("a").unwrap();
        let (code, shifted) = keycode_for_keysym(&mapping, keysym).unwrap();
        assert!(code > 0);
        assert!(!shifted);
    }

    #[test]
    fn keycode_for_keysym_finds_shifted_slot() {
        let (conn, _) = open_connection(":0").expect("need local display");
        let mapping = get_mapping(&conn).expect("keyboard mapping");
        let keysym = keysym_for_name("A").unwrap();
        let (code, shifted) = keycode_for_keysym(&mapping, keysym).unwrap();
        assert!(code > 0);
        assert!(shifted);
    }
}
