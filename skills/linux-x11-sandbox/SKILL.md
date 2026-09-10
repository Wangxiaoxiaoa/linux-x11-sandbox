---
name: linux-x11-sandbox
description: Run and automate Linux GUI applications in isolated X11 displays. Use when you need to launch a GUI app, interact with it via mouse/keyboard, take screenshots, or inspect the AT-SPI accessibility tree without affecting the host desktop.
license: MIT
compatibility: Linux with xvfb and openbox installed. Optional host X display required for visible Xephyr backend.
---

# linux-x11-sandbox

`linux-x11-sandbox` provides isolated X11 displays and a built-in automation driver exposed through an MCP stdio server.

## When to use

- Launch a GUI application in a clean environment.
- Take screenshots of an app for verification.
- Send mouse/keyboard input to an app without touching the host desktop.
- Read the AT-SPI accessibility tree or perform actions on UI elements.
- Run multiple independent displays at the same time.

## Setup

Build the MCP server once:

```bash
cd ../../../
cargo build --release
```

Verify system dependencies:

```bash
which xvfb openbox
```

If missing on Debian/Ubuntu:

```bash
sudo apt-get install -y xvfb openbox
```

## Start the MCP server

Run the binary from the project root. It speaks MCP over stdin/stdout.

```bash
cd ../../../
./target/release/linux-x11-sandbox
```

All communication uses JSON-RPC 2.0.

## Core workflow

1. **Initialize** the MCP connection.
2. **Create a display** with `lxs_display_create`. Default backend is headless `Xvfb`. Use `{"backend": "xephyr"}` for a visible window (requires `DISPLAY` to point to a host X server).
3. **Launch an app** with `lxs_app_launch`.
4. **Interact**: click, type, screenshot, or read the AT-SPI tree.
5. **Terminate apps** and **destroy the display** to clean up.

## Tool reference

| Tool | Purpose |
|------|---------|
| `lxs_display_create` | Create display. Args: `backend` (`xvfb` or `xephyr`). Returns `display_id` and `display`. |
| `lxs_display_destroy` | Destroy display and all its apps. Args: `display_id`. |
| `lxs_display_info` | Get resolution and app count. Args: `display_id`. |
| `lxs_app_launch` | Launch a command. Args: `display_id`, `command`, `args` (array). Returns `pid`. |
| `lxs_app_terminate` | Kill app by PID. Args: `display_id`, `pid`. |
| `lxs_app_list` | List app PIDs. Args: `display_id`. |
| `lxs_input_click` | Click at `(x, y)`. Args: `display_id`, `x`, `y`, `button` (`left`/`middle`/`right`), `count` (optional). |
| `lxs_input_move` | Move cursor to `(x, y)`. |
| `lxs_input_scroll` | Scroll. Args: `dx`, `dy`. |
| `lxs_input_drag` | Drag from `(x1, y1)` to `(x2, y2)`. |
| `lxs_input_get_cursor_position` | Get current cursor position. |
| `lxs_input_type` | Type text. Arg: `text`. |
| `lxs_input_key` | Press key or combo. Args: `key`, `modifiers` (array). |
| `lxs_capture_screenshot` | Base64 PNG screenshot. |
| `lxs_capture_region` | Region screenshot. Args: `x`, `y`, `width`, `height`. |
| `lxs_window_focus` | Focus the active window. |
| `lxs_window_raise` | Raise the active window. |
| `lxs_window_resize` | Resize active window. Args: `width`, `height`. |
| `lxs_window_move` | Move active window. Args: `x`, `y`. |
| `lxs_window_list` | List top-level windows. |
| `lxs_clipboard_get` | Get clipboard text. |
| `lxs_clipboard_set` | Set clipboard text. Arg: `text`. |
| `lxs_wait` | Wait. Arg: `ms`. |
| `lxs_state_window` | Active window title. Returns `{"title": "..."}` or `null`. |
| `lxs_state_tree` | AT-SPI tree. Returns elements with `index`, `role`, `name`, `actions`. |
| `lxs_state_element_bounds` | Bounds of element by `element_index`. |
| `lxs_perform_action` | Perform AT-SPI action. Args: `pid`, `element_index`, `action`. |

## Example: launch and screenshot

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

## Example: click an AT-SPI element

1. Get the tree:

```json
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"lxs_state_tree","arguments":{"display_id":"d-99","pid":12345}}}
```

2. Pick an element with a non-empty `actions` list.
3. Call `lxs_perform_action` with `pid`, `element_index`, and one of the supported `action` names.

## Tips

- Always destroy displays when done to free the X server and application processes.
- Use `lxs_input_click` before `lxs_input_type` or `lxs_input_key` when the target window needs focus.
- Some apps (e.g., Chromium) need environment variables to enable AT-SPI; the runtime injects these automatically.
- For headed debugging, use `{"backend": "xephyr"}` and set `DISPLAY` to a running host X server.

## More documentation

- English README: [README.md](../../../README.md)
- 中文 README: [README.zh-CN.md](../../../README.zh-CN.md)
- Architecture: [docs/ARCHITECTURE.md](../../../docs/ARCHITECTURE.md)
