<div align="center">

# linux-x11-sandbox

Linux X11 GUI sandbox with a built-in native automation driver.

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## What is this?

`linux-x11-sandbox` is a **lightweight Linux GUI automation sandbox**. It lets AI agents and programs run real GUI applications — such as browsers, terminals, IDEs, and office suites — inside isolated X11 displays, and interact with them through mouse, keyboard, screenshots, and the AT-SPI accessibility tree.

Unlike a full virtual machine or container, it focuses on the **display and input layer**: each sandbox gets its own X server, window manager, and application process group. Sandboxes do not share windows, focus, clipboard, or desktop shell, while still running on the same filesystem as the host. This makes it fast to start, cheap to run, and easy to compose with existing isolation technologies.

Typical use cases:

- Let an AI agent operate a GUI app without taking over the host desktop.
- Run end-to-end tests that need a real screen, real input, and real window focus.
- Capture screenshots or accessibility trees for model training or verification.
- Run multiple independent GUI sessions on the same machine.

It can be used in three ways:

1. **MCP server** — plug into any agent that supports the [Model Context Protocol](https://modelcontextprotocol.io/).
2. **Rust SDK** — embed the sandbox and driver directly in your Rust project.
3. **Composable sandbox layer** — run inside an existing container/VM sandbox (Docker, e2b, Daytona, etc.) to add X11 automation capabilities.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for crate layout, trait design, and integration patterns.

## Quick install

Install the Python distribution package:

```bash
pip install linux-x11-sandbox
```

Then register the bundled skill and MCP server with your agents:

```bash
linux-x11-sandbox setup
```

Restart your agent. The setup command auto-detects pi, Claude Code, Codex,
Claude Desktop, Cursor, and `.agents`.

Requirements:

- Python 3.10+
- Rust toolchain
- Linux with `xvfb` and `openbox`

## Manual build

Requirements:

- Rust toolchain
- `xvfb`
- `openbox`

On Debian/Ubuntu:

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

The MCP server binary is at `target/release/linux-x11-sandbox`.

---

## 1. Use as an MCP server

The MCP server speaks JSON-RPC 2.0 over stdin/stdout. Any MCP-compatible agent can launch it.

### 1.1 Launch the server

If installed via pip:

```bash
linux-x11-sandbox
```

Or from the built Rust binary:

```bash
./target/release/linux-x11-sandbox
```

### 1.2 One-click agent registration

```bash
linux-x11-sandbox setup
```

This registers the skill and MCP server for detected agents. Restart your agent after running it.

### 1.3 Manual registration

The skill lives in [`skills/linux-x11-sandbox/SKILL.md`](skills/linux-x11-sandbox/SKILL.md). Symlink it into your agent's skill directory:

- **pi**: `~/.pi/agent/skills/linux-x11-sandbox/`
- **Claude Code**: `.claude/skills/linux-x11-sandbox/`
- **Codex**: `.codex/skills/linux-x11-sandbox/`

For MCP, most agents use an `mcpServers` block. Example for Claude Desktop:

```json
{
  "mcpServers": {
    "linux-x11-sandbox": {
      "command": "linux-x11-sandbox"
    }
  }
}
```

### 1.4 MCP tools

Once connected, the agent can call:

| Tool | Description |
|------|-------------|
| `lxs_display_create` | Create display (`backend`: `xvfb` or `xephyr`) |
| `lxs_display_destroy` | Destroy display and apps |
| `lxs_display_info` | Resolution and app count |
| `lxs_app_launch` | Launch an application |
| `lxs_app_terminate` | Terminate by PID |
| `lxs_app_list` | List app PIDs |
| `lxs_input_click` | Click at `(x, y)` with optional `button` and `count` |
| `lxs_input_move` | Move cursor |
| `lxs_input_scroll` | Scroll |
| `lxs_input_drag` | Drag from `(x1, y1)` to `(x2, y2)` |
| `lxs_input_get_cursor_position` | Get current mouse position |
| `lxs_input_type` | Type text |
| `lxs_input_key` | Key or combo |
| `lxs_capture_screenshot` | Full screenshot |
| `lxs_capture_region` | Region screenshot |
| `lxs_window_focus` | Focus the active window |
| `lxs_window_raise` | Raise the active window |
| `lxs_window_resize` | Resize the active window |
| `lxs_window_move` | Move the active window |
| `lxs_window_list` | List top-level windows |
| `lxs_clipboard_get` | Get clipboard text |
| `lxs_clipboard_set` | Set clipboard text |
| `lxs_wait` | Wait for `ms` milliseconds |
| `lxs_state_window` | Active window title |
| `lxs_state_tree` | AT-SPI tree |
| `lxs_state_element_bounds` | Element bounds |
| `lxs_perform_action` | AT-SPI action |

### 1.5 Example session

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

---

## 2. Use as a Rust SDK

Add the crates you need to your `Cargo.toml`:

```toml
[dependencies]
lxs-core = { path = "path/to/linux-x11-sandbox/lxs/core" }
lxs-runtime = { path = "path/to/linux-x11-sandbox/lxs/runtime" }
lxs-driver = { path = "path/to/linux-x11-sandbox/lxs/driver" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Example:

```rust
use lxs_core::{Driver, Rect};
use lxs_runtime::{DisplayConfig, Runtime};
use lxs_driver::NativeDriver;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = Runtime::new();
    let display = runtime.create_display(DisplayConfig::default()).await?;
    let id = display.id().to_string();

    display.launch_app("xterm", &[]).await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let driver = NativeDriver::new(display.display())?;
    driver.click(100, 100, lxs_core::MouseButton::Left, 1).await?;
    let screenshot = driver.screenshot().await?;
    std::fs::write("screenshot.png", screenshot.data)?;

    display.destroy().await?;
    Ok(())
}
```

The `Driver` trait in `lxs-core` allows swapping `NativeDriver` with other backends (for example, a future `CuaDriver` adapter).

---

## 3. Use as a composable sandbox layer

`linux-x11-sandbox` does **not** replace general-purpose container or VM sandboxes. Instead, it adds an X11 automation layer inside them.

Typical deployment:

```
Docker / e2b / Daytona / VM
└── linux-x11-sandbox
    ├── Xvfb or Xephyr
    ├── openbox
    └── target application
```

This keeps filesystem/network isolation handled by the outer sandbox while giving the agent a real GUI environment to interact with.

### 3.1 Docker

Build the image:

```bash
docker build -t linux-x11-sandbox .
```

Run a persistent container:

```bash
docker compose up -d
```

Use the bundled bridge script as the MCP server in your agent config:

```json
{
  "mcpServers": {
    "linux-x11-sandbox": {
      "command": "/path/to/linux-x11-sandbox/scripts/docker-mcp.sh"
    }
  }
}
```

Or invoke directly through `docker exec`:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | docker exec -i linux-x11-sandbox linux-x11-sandbox
```

### 3.2 One-off container

```bash
docker run -i --rm linux-x11-sandbox
```

Then send MCP JSON-RPC over stdin.

---

## Testing

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

Integration tests run sequentially to avoid display-number conflicts.

## License

MIT
