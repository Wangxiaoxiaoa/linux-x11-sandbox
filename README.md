<div align="center">

# linux-x11-harness

**A self-contained Linux X11 GUI harness for automation, testing, and AI agents.**

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## What is this?

`linux-x11-harness` runs GUI applications inside isolated X11 displays and lets agents interact with them through mouse, keyboard, screenshots, and the AT-SPI accessibility tree.

Each harness gets its own X server, window manager, and application process group. Harnesses do not share windows, focus, or clipboard. All 24 automation tools have real-effect integration tests that exercise real X11 input, window changes, screenshots, and accessibility actions.

Typical uses:

- Let an agent operate a GUI app without taking over the host desktop.
- Run end-to-end tests that need real screen, input, and window focus.
- Capture screenshots or accessibility trees for verification.

It can be used as an **MCP server**, a **Rust SDK**, or a **composable harness layer** inside Docker/VMs.

## Features

| Feature | Description |
| --- | --- |
| **Isolated displays** | Each harness is a separate `DISPLAY` with its own X server and window manager. |
| **24 automation tools** | Mouse, keyboard, window, element, capture, clipboard, state, and lifecycle actions. |
| **MCP server** | Daemon + stdio proxy; plug into any MCP-compatible agent. |
| **Rust SDK** | Compose `Runtime`, `Display`, and `Driver` directly in Rust. |
| **Docker support** | Ready-to-use `Dockerfile` and `docker-compose.yml`. |
| **Headless or headed** | Use `xvfb` in CI or `xephyr` for visual debugging. |

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for crate layout and design goals.

## Quick start

```bash
pip install linux-x11-harness
linux-x11-harness setup
```

Requirements: Python 3.10+, Rust toolchain, Linux with `xvfb` and `openbox`.

## Manual build

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

Binary: `target/release/linux-x11-harness`.

---

## Use as an MCP server

A long-lived `linux-x11-harness serve` daemon owns displays and drivers. Each agent runs a lightweight `linux-x11-harness mcp` stdio proxy that forwards JSON-RPC to the daemon over a Unix socket.

### Launch

```bash
./target/release/linux-x11-harness serve   # start daemon
./target/release/linux-x11-harness mcp     # stdio proxy (auto-starts daemon)
./target/release/linux-x11-harness status  # check daemon
./target/release/linux-x11-harness stop    # stop daemon
```

### Tool categories

#### Display & application lifecycle

| Tool | Description |
| --- | --- |
| `lxh_display_create` | Create display (`backend`: `xvfb` or `xephyr`, `persistent`: bool) |
| `lxh_display_attach` | Attach to an existing display (e.g. `:0`) |
| `lxh_display_detach` | Detach from an existing display without destroying it |
| `lxh_display_destroy` | Destroy display |
| `lxh_display_info` | Resolution and app count |
| `lxh_app_launch` | Launch an application |
| `lxh_app_terminate` | Terminate by PID |
| `lxh_wait` | Wait for `ms` milliseconds |

#### Input

| Tool | Description |
| --- | --- |
| `lxh_input_click` | Click at `(x, y)` with `button` (`left`/`right`/`middle`) and `count` |
| `lxh_input_move` | Move cursor |
| `lxh_input_scroll` | Scroll |
| `lxh_input_drag` | Drag from `(x1, y1)` to `(x2, y2)` |
| `lxh_input_get_cursor_position` | Get current mouse position |
| `lxh_input_type` | Type text |
| `lxh_input_key` | Key or combo |

#### Window management

| Tool | Description |
| --- | --- |
| `lxh_window_focus` | Focus a window by `window_id` |
| `lxh_window_set_frame` | Set a window's position and size |
| `lxh_window_close` | Close a window by `window_id` |

#### Capture, clipboard, and state

| Tool | Description |
| --- | --- |
| `lxh_capture_screenshot` | Full screenshot |
| `lxh_capture_window` | Screenshot a specific window |
| `lxh_clipboard_get` | Get clipboard text |
| `lxh_clipboard_set` | Set clipboard text |
| `lxh_get_desktop_overview` | Desktop overview: processes and windows |
| `lxh_get_window_state` | Window metadata + optional tree + optional screenshot |

#### Accessibility actions

| Tool | Description |
| --- | --- |
| `lxh_set_value` | Set AT-SPI editable element value |
| `lxh_click_element` | Click an AT-SPI element by `pid` and `index` |

### Example session

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxh_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxh_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxh_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxh_display_destroy","arguments":{"display_id":"d-99"}}}
```

---

## Use as a Rust SDK

```toml
[dependencies]
lxh-core = { path = "path/to/linux-x11-harness/lxs/core" }
lxh-runtime = { path = "path/to/linux-x11-harness/lxs/runtime" }
lxh-driver = { path = "path/to/linux-x11-harness/lxs/driver" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use lxh_core::{Driver, MouseButton};
use lxh_runtime::{DisplayConfig, Runtime};
use lxh_driver::NativeDriver;

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
docker build -t linux-x11-harness .
docker compose up -d
```

Use `scripts/docker-mcp.sh` as the MCP command, or run `docker exec -i <container> linux-x11-harness`.

---

## Testing

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

## License

MIT
