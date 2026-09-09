<div align="center">

# linux-x11-sandbox

Linux X11 GUI sandbox with a built-in native automation driver.

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

`linux-x11-sandbox` runs GUI applications in isolated X11 displays. Each sandbox gets its own X server, window manager, and set of applications. Displays do not share windows, focus, clipboard, or desktop shell—only the underlying filesystem.

It is designed to be used in three ways:

1. **MCP server** — plug into any agent that supports the [Model Context Protocol](https://modelcontextprotocol.io/).
2. **Rust SDK** — embed the sandbox and driver directly in your Rust project.
3. **Composable sandbox layer** — run inside an existing container/VM sandbox (Docker, e2b, Daytona, etc.) to add X11 automation capabilities.

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

The MCP server binary is at `target/release/linux-x11-sandbox`.

---

## 1. Use as an MCP server

The MCP server speaks JSON-RPC 2.0 over stdin/stdout. Any MCP-compatible agent can launch it.

### 1.1 Launch the server

```bash
./target/release/linux-x11-sandbox
```

### 1.2 Register the skill

A ready-to-use skill is included in [`skills/linux-x11-sandbox/SKILL.md`](skills/linux-x11-sandbox/SKILL.md). Copy or symlink it into your agent's skill directory:

- **pi**: `~/.pi/agent/skills/linux-x11-sandbox/` or `.pi/skills/linux-x11-sandbox/`
- **Claude Code**: `.claude/skills/linux-x11-sandbox/`
- **Codex**: `.codex/skills/linux-x11-sandbox/`

```bash
mkdir -p ~/.pi/agent/skills
ln -s /path/to/linux-x11-sandbox/skills/linux-x11-sandbox ~/.pi/agent/skills/linux-x11-sandbox
```

### 1.3 Register the server in an agent

Most agents support an `mcpServers` configuration. Example for Claude Desktop:

```json
{
  "mcpServers": {
    "linux-x11-sandbox": {
      "command": "/path/to/linux-x11-sandbox/target/release/linux-x11-sandbox"
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
| `lxs_input_click` | Click at `(x, y)` |
| `lxs_input_move` | Move cursor |
| `lxs_input_scroll` | Scroll |
| `lxs_input_type` | Type text |
| `lxs_input_key` | Key or combo |
| `lxs_capture_screenshot` | Full screenshot |
| `lxs_capture_region` | Region screenshot |
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

The host only needs to:

1. Provide the sandbox image with `xvfb`, `openbox`, and the built binary.
2. Expose the MCP stdio server to the agent.

This keeps filesystem/network isolation handled by the outer sandbox while giving the agent a real GUI environment to interact with.

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
