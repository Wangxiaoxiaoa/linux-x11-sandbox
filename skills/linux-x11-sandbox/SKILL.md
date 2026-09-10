---
name: linux-x11-sandbox
description: Run and automate Linux GUI applications in isolated X11 displays.
license: MIT
compatibility: Linux with xvfb and openbox installed.
---

# linux-x11-sandbox

Isolated X11 displays with a built-in automation driver, exposed through an MCP stdio server.

## When to use

- Launch a GUI application in a clean environment.
- Take screenshots for verification.
- Send mouse/keyboard input without touching the host desktop.
- Read the AT-SPI accessibility tree or perform actions on UI elements.

## Setup

```bash
cd ../../../
cargo build --release
```

Requires `xvfb` and `openbox`.

## Start the MCP server

```bash
cd ../../../
./target/release/linux-x11-sandbox
```

## Core workflow

1. Initialize the MCP connection.
2. Create a display with `lxs_display_create` (default `xvfb`, use `{"backend": "xephyr"}` for visible).
3. Launch an app with `lxs_app_launch`.
4. Interact: click, type, screenshot, read AT-SPI tree.
5. Terminate apps and destroy the display.

## Tool reference

| Tool | Purpose |
|------|---------|
| `lxs_display_create` | Args: `backend` (`xvfb`/`xephyr`). Returns `display_id`, `display`. |
| `lxs_display_destroy` | Args: `display_id`. |
| `lxs_display_info` | Args: `display_id`. Returns `display`, `width`, `height`, `app_count`. |
| `lxs_app_launch` | Args: `display_id`, `command`, `args` (array). Returns `pid`. |
| `lxs_app_terminate` | Args: `display_id`, `pid`. |
| `lxs_input_click` | Args: `display_id`, `x`, `y`, optional `button` (`left`/`right`/`middle`), optional `count`. |
| `lxs_input_move` | Args: `display_id`, `x`, `y`. |
| `lxs_input_scroll` | Args: `display_id`, `dx`, `dy`. |
| `lxs_input_drag` | Args: `display_id`, `x1`, `y1`, `x2`, `y2`. |
| `lxs_input_get_cursor_position` | Args: `display_id`. Returns `x`, `y`. |
| `lxs_input_type` | Args: `display_id`, `text`. |
| `lxs_input_key` | Args: `display_id`, `key`, `modifiers` (array). |
| `lxs_capture_screenshot` | Args: `display_id`. Returns base64 PNG. |
| `lxs_capture_window` | Args: `display_id`, `window_id`. Returns base64 PNG. |
| `lxs_window_focus` | Args: `display_id`, `window_id`. |
| `lxs_window_set_frame` | Args: `display_id`, `window_id`, `x`, `y`, `width`, `height`. |
| `lxs_window_close` | Args: `display_id`, `window_id`. |
| `lxs_clipboard_get` | Args: `display_id`. Returns `text`. |
| `lxs_clipboard_set` | Args: `display_id`, `text`. |
| `lxs_get_desktop_overview` | Args: `display_id`. Returns `processes` (`pid`, `name`) and `windows` (`window_id`, `pid`, `title`, `bounds`). |
| `lxs_get_window_state` | Args: `display_id`, `pid`, `window_id`, `include_tree` (default true), `include_screenshot` (default false). Returns `window_id`, `title`, `app_name`, `bounds`, optional `tree`, optional `screenshot` (base64 PNG). |
| `lxs_set_value` | Args: `display_id`, `pid`, `index`, `value`. |
| `lxs_click_element` | Args: `display_id`, `pid`, `index`, optional `button` (`left`/`right`/`middle`). |
| `lxs_wait` | Args: `ms`. |

## Example

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

## Tips

- Always destroy displays when done.
- Click inside a window before typing if the target needs focus.
- For AT-SPI, launch apps with their own accessibility flags if needed.
