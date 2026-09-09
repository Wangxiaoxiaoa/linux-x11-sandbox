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
                .map_err(xerr)?;

            let button = match button {
                MouseButton::Left => 1,
                MouseButton::Middle => 2,
                MouseButton::Right => 3,
            };

            for _ in 0..count {
                conn.xtest_fake_input(BUTTON_PRESS_EVENT, button, 0, root, x16, y16, 0)
                    .map_err(xerr)?;
                conn.xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, root, x16, y16, 0)
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
                    .map_err(xerr)?;
                conn.xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, root, 0, 0, 0)
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
            let mapping = conn
                .get_keyboard_mapping(8, 248)
                .map_err(xerr)?
                .reply()
                .map_err(xerr)?;

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
                    conn.xtest_fake_input(KEY_PRESS_EVENT, shift_kc, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(xerr)?;
                    conn.xtest_fake_input(KEY_PRESS_EVENT, keycode, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(xerr)?;
                    conn.xtest_fake_input(KEY_RELEASE_EVENT, keycode, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(xerr)?;
                    conn.xtest_fake_input(KEY_RELEASE_EVENT, shift_kc, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(xerr)?;
                } else {
                    conn.xtest_fake_input(KEY_PRESS_EVENT, keycode, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(xerr)?;
                    conn.xtest_fake_input(KEY_RELEASE_EVENT, keycode, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(xerr)?;
                }
                conn.flush().map_err(xerr)?;
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
            let (conn, _) = open_connection(&display)?;
            let mapping = conn
                .get_keyboard_mapping(8, 248)
                .map_err(xerr)?
                .reply()
                .map_err(xerr)?;

            let keysym = keysym_for_name(&key)
                .or_else(|| key.chars().next().and_then(keysym_for_char))
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
                conn.xtest_fake_input(KEY_PRESS_EVENT, code, 0, x11rb::NONE, 0, 0, 0)
                    .map_err(xerr)?;
            }
            for &code in codes.iter().rev() {
                conn.xtest_fake_input(KEY_RELEASE_EVENT, code, 0, x11rb::NONE, 0, 0, 0)
                    .map_err(xerr)?;
            }

            conn.flush().map_err(xerr)?;
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
        assert_eq!(keysym_for_char('a'), Some(0x61));
        assert_eq!(keysym_for_char('A'), Some(0x41));
        assert_eq!(keysym_for_name("Return"), Some(0xff0d));
        assert_eq!(keysym_for_name("Shift_L"), Some(0xffe1));
    }

    #[test]
    fn keycode_for_keysym_finds_slot_zero() {
        let mapping = x11rb::protocol::xproto::GetKeyboardMappingReply {
            keysyms_per_keycode: 2,
            sequence: 0,
            keysyms: vec![
                0x61, 0x41, // keycode 8 -> a/A
                0x62, 0x42, // keycode 9 -> b/B
            ],
        };
        assert_eq!(keycode_for_keysym(&mapping, 0x61), Some((8, false)));
    }

    #[test]
    fn keycode_for_keysym_finds_shifted_slot() {
        let mapping = x11rb::protocol::xproto::GetKeyboardMappingReply {
            keysyms_per_keycode: 2,
            sequence: 0,
            keysyms: vec![
                0x61, 0x41, // keycode 8 -> a/A
                0x62, 0x42, // keycode 9 -> b/B
            ],
        };
        assert_eq!(keycode_for_keysym(&mapping, 0x41), Some((8, true)));
    }
}
