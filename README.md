<div align="center">

# linux-x11-sandbox

Linux X11 GUI sandbox with a built-in native automation driver.

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

`linux-x11-sandbox` runs GUI applications in isolated X11 displays. Each sandbox gets its own X server, window manager, and set of applications. Displays do not share windows, focus, clipboard, or desktop shell—only the underlying filesystem.

## Features

- **Isolated X11 display per sandbox** — headless `Xvfb` or visible `Xephyr` + `openbox`
- **Built-in native automation driver**
  - Mouse: move, click, scroll
  - Keyboard: type text, key combinations
  - Screenshot: full screen and region
  - AT-SPI accessibility tree, element bounds, action injection
- **MCP stdio server** for agent integration
- **Rust SDK** for embedding

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for crate layout, trait design, and integration patterns.

## Build

Requirements:

- Rust toolchain
- `xvfb`
- `openbox`

On Debian/Ubuntu:

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

The binary is at `target/release/linux-x11-sandbox`.

## Run the MCP server

```bash
./target/release/linux-x11-sandbox
```

The server speaks [MCP](https://modelcontextprotocol.io/) over stdio.

## Quick start

Create a headless display and launch an application:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"demo","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

Create a visible display with Xephyr (requires a host X display, e.g. `DISPLAY=:0`):

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{"backend":"xephyr"}}}
```

## MCP tools

| Tool | Description |
|------|-------------|
| `lxs_display_create` | Create a new X11 display (`backend`: `xvfb` or `xephyr`) |
| `lxs_display_destroy` | Destroy a display and its applications |
| `lxs_display_info` | Get display resolution and application count |
| `lxs_app_launch` | Launch an application on a display |
| `lxs_app_terminate` | Terminate an application by PID |
| `lxs_app_list` | List running applications on a display |
| `lxs_input_click` | Click at screen coordinates |
| `lxs_input_move` | Move the mouse cursor |
| `lxs_input_scroll` | Scroll by a delta |
| `lxs_input_type` | Type text |
| `lxs_input_key` | Press a key or key combination |
| `lxs_capture_screenshot` | Take a screenshot |
| `lxs_capture_region` | Take a screenshot of a region |
| `lxs_state_window` | Get the active window title |
| `lxs_state_tree` | Walk the AT-SPI accessibility tree |
| `lxs_state_element_bounds` | Get bounds of an indexed element |
| `lxs_perform_action` | Perform an AT-SPI action on an element |

## Testing

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

Integration tests run sequentially to avoid display-number conflicts.

## License

MIT
