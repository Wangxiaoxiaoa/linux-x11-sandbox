---
name: linux-x11-harness
description: Run and automate Linux GUI applications in isolated X11 displays.
license: MIT
compatibility: Linux with xvfb and openbox installed.
---

# linux-x11-harness

Isolated X11 displays with a built-in automation driver, exposed through a daemon + MCP stdio proxy.

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
./target/release/linux-x11-harness serve   # start daemon
./target/release/linux-x11-harness mcp     # stdio proxy (auto-starts daemon)
```

### Isolated daemon instances

By default, all agents share one daemon and each MCP connection gets its own session. Displays created by one session are cleaned up when that session disconnects.

If you need a fully separate daemon process (for stronger isolation, independent lifecycle, or a dedicated environment), use `--socket`:

```bash
./target/release/linux-x11-harness mcp --socket /tmp/lxh-agent-<id>.sock
```

The displays for that socket live only in that daemon instance and are cleaned up when the connection ends.

## Core workflow

1. Initialize the MCP connection.
2. Create a display with `lxh_display_create` (default `xvfb`, use `{"backend": "xephyr"}` for visible).
3. Launch an app with `lxh_app_launch`.
4. Interact: click, type, screenshot, read AT-SPI tree.
5. Terminate apps and destroy the display.

## Tool reference

| Tool | Purpose |
|------|---------|
| `lxh_display_create` | Args: `backend` (`xvfb`/`xephyr`), optional `persistent`. Returns `display_id`, `display`. |
| `lxh_display_attach` | Args: `display_id` (e.g. `:0`). Attach to an existing display. |
| `lxh_display_detach` | Args: `display_id`. Detach without destroying. |
| `lxh_display_destroy` | Args: `display_id`. |
| `lxh_display_info` | Args: `display_id`. Returns `display`, `width`, `height`, `app_count`. |
| `lxh_app_launch` | Args: `display_id`, `command`, `args` (array). Returns `pid`. |
| `lxh_app_terminate` | Args: `display_id`, `pid`. |
| `lxh_input_click` | Args: `display_id`, `x`, `y`, optional `button` (`left`/`right`/`middle`), optional `count`. |
| `lxh_input_move` | Args: `display_id`, `x`, `y`. |
| `lxh_input_scroll` | Args: `display_id`, `dx`, `dy`. |
| `lxh_input_drag` | Args: `display_id`, `x1`, `y1`, `x2`, `y2`. |
| `lxh_input_get_cursor_position` | Args: `display_id`. Returns `x`, `y`. |
| `lxh_input_type` | Args: `display_id`, `text`. |
| `lxh_input_key` | Args: `display_id`, `key`, `modifiers` (array). |
| `lxh_capture_screenshot` | Args: `display_id`. Returns base64 PNG. |
| `lxh_capture_window` | Args: `display_id`, `window_id`. Returns base64 PNG. |
| `lxh_window_focus` | Args: `display_id`, `window_id`. |
| `lxh_window_set_frame` | Args: `display_id`, `window_id`, `x`, `y`, `width`, `height`. |
| `lxh_window_close` | Args: `display_id`, `window_id`. |
| `lxh_clipboard_get` | Args: `display_id`. Returns `text`. |
| `lxh_clipboard_set` | Args: `display_id`, `text`. |
| `lxh_get_desktop_overview` | Args: `display_id`. Returns `processes` (`pid`, `name`) and `windows` (`window_id`, `pid`, `title`, `bounds`). |
| `lxh_get_window_state` | Args: `display_id`, `pid`, `window_id`, `include_tree` (default true), `include_screenshot` (default false). Returns `window_id`, `title`, `app_name`, `bounds`, optional `tree`, optional `screenshot` (base64 PNG). |
| `lxh_set_value` | Args: `display_id`, `pid`, `index`, `value`. |
| `lxh_click_element` | Args: `display_id`, `pid`, `index`, optional `button` (`left`/`right`/`middle`). |
| `lxh_wait` | Args: `ms`. |

## Example

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxh_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxh_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxh_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxh_display_destroy","arguments":{"display_id":"d-99"}}}
```

## Tips

- Always destroy displays when done.
- Click inside a window before typing if the target needs focus.
- For AT-SPI, launch apps with their own accessibility flags if needed.
