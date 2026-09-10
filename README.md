<div align="center">

# linux-x11-sandbox

**A self-contained Linux X11 GUI sandbox for automation, testing, and AI agents.**

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## What is this?

`linux-x11-sandbox` runs GUI applications inside isolated X11 displays and lets agents interact with them through mouse, keyboard, screenshots, and the AT-SPI accessibility tree.

Each sandbox gets its own X server, window manager, and application process group. Sandboxes do not share windows, focus, or clipboard. All 24 automation tools have real-effect integration tests that exercise real X11 input, window changes, screenshots, and accessibility actions.

Typical uses:

- Let an agent operate a GUI app without taking over the host desktop.
- Run end-to-end tests that need real screen, input, and window focus.
- Capture screenshots or accessibility trees for verification.

It can be used as an **MCP server**, a **Rust SDK**, or a **composable sandbox layer** inside Docker/VMs.

## Features

| Feature | Description |
| --- | --- |
| **Isolated displays** | Each sandbox is a separate `DISPLAY` with its own X server and window manager. |
| **24 automation tools** | Mouse, keyboard, window, element, capture, clipboard, state, and lifecycle actions. |
| **MCP server** | JSON-RPC over stdio; plug into any MCP-compatible agent. |
| **Rust SDK** | Compose `Runtime`, `Display`, and `Driver` directly in Rust. |
| **Docker support** | Ready-to-use `Dockerfile` and `docker-compose.yml`. |
| **Headless or headed** | Use `xvfb` in CI or `xephyr` for visual debugging. |

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for crate layout and design goals.

## Quick start

```bash
pip install linux-x11-sandbox
linux-x11-sandbox setup
```

Requirements: Python 3.10+, Rust toolchain, Linux with `xvfb` and `openbox`.

## Manual build

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

Binary: `target/release/linux-x11-sandbox`.

---

## Use as an MCP server

The server speaks JSON-RPC 2.0 over stdin/stdout.

### Launch

```bash
./target/release/linux-x11-sandbox
```

### Tool categories

#### Display & application lifecycle

| Tool | Description |
| --- | --- |
| `lxs_display_create` | Create display (`backend`: `xvfb` or `xephyr`) |
| `lxs_display_destroy` | Destroy display |
| `lxs_display_info` | Resolution and app count |
| `lxs_app_launch` | Launch an application |
| `lxs_app_terminate` | Terminate by PID |
| `lxs_wait` | Wait for `ms` milliseconds |

#### Input

| Tool | Description |
| --- | --- |
| `lxs_input_click` | Click at `(x, y)` with `button` (`left`/`right`/`middle`) and `count` |
| `lxs_input_move` | Move cursor |
| `lxs_input_scroll` | Scroll |
| `lxs_input_drag` | Drag from `(x1, y1)` to `(x2, y2)` |
| `lxs_input_get_cursor_position` | Get current mouse position |
| `lxs_input_type` | Type text |
| `lxs_input_key` | Key or combo |

#### Window management

| Tool | Description |
| --- | --- |
| `lxs_window_focus` | Focus a window by `window_id` |
| `lxs_window_set_frame` | Set a window's position and size |
| `lxs_window_close` | Close a window by `window_id` |

#### Capture, clipboard, and state

| Tool | Description |
| --- | --- |
| `lxs_capture_screenshot` | Full screenshot |
| `lxs_capture_window` | Screenshot a specific window |
| `lxs_clipboard_get` | Get clipboard text |
| `lxs_clipboard_set` | Set clipboard text |
| `lxs_get_desktop_overview` | Desktop overview: processes and windows |
| `lxs_get_window_state` | Window metadata + optional tree + optional screenshot |

#### Accessibility actions

| Tool | Description |
| --- | --- |
| `lxs_set_value` | Set AT-SPI editable element value |
| `lxs_click_element` | Click an AT-SPI element by `pid` and `index` |

### Example session

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

---

## Use as a Rust SDK

```toml
[dependencies]
lxs-core = { path = "path/to/linux-x11-sandbox/lxs/core" }
lxs-runtime = { path = "path/to/linux-x11-sandbox/lxs/runtime" }
lxs-driver = { path = "path/to/linux-x11-sandbox/lxs/driver" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use lxs_core::{Driver, MouseButton};
use lxs_runtime::{DisplayConfig, Runtime};
use lxs_driver::NativeDriver;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = Runtime::new();
    let display = runtime.create_display(DisplayConfig::default()).await?;

    display.launch_app("xterm", &[]).await?;
    let driver = NativeDriver::new(display.display())?;
    driver.click(100, 100, MouseButton::Left, 1).await?;
    let screenshot = driver.screenshot().await?;
    std::fs::write("screenshot.png", screenshot.data)?;

    display.destroy().await?;
    Ok(())
}
```

---

## Docker

```bash
docker build -t linux-x11-sandbox .
docker compose up -d
```

Use `scripts/docker-mcp.sh` as the MCP command, or run `docker exec -i <container> linux-x11-sandbox`.

---

## Testing

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

## License

MIT
