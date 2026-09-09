<div align="center">

# linux-x11-sandbox

Linux X11 GUI sandbox with a built-in native automation driver.

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](#english) · [中文](#中文)

</div>

---

## English

`linux-x11-sandbox` runs GUI applications in isolated X11 displays. Each sandbox gets its own X server, window manager, and set of applications. Displays do not share windows, focus, clipboard, or desktop shell—only the underlying filesystem.

### Features

- **Isolated X11 display per sandbox** — headless `Xvfb` or visible `Xephyr` + `openbox`
- **Built-in native automation driver**
  - Mouse: move, click, scroll
  - Keyboard: type text, key combinations
  - Screenshot: full screen and region
  - AT-SPI accessibility tree, element bounds, action injection
- **MCP stdio server** for agent integration
- **Rust SDK** for embedding

### Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for crate layout, trait design, and integration patterns.

### Build

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

### Run the MCP server

```bash
./target/release/linux-x11-sandbox
```

The server speaks [MCP](https://modelcontextprotocol.io/) over stdio.

### Quick start

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

### MCP tools

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

### Testing

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

Integration tests run sequentially to avoid display-number conflicts.

---

## 中文

`linux-x11-sandbox` 在独立的 X11 display 中运行 GUI 应用。每个沙盒拥有独立的 X 服务器、窗口管理器和应用进程。display 之间不共享窗口、焦点、剪贴板或桌面 shell，只共享底层文件系统。

### 特性

- **每个沙盒一个独立 X11 display** —— 无头 `Xvfb` 或可见 `Xephyr` + `openbox`
- **内置原生自动化驱动**
  - 鼠标：移动、点击、滚动
  - 键盘：输入文本、组合键
  - 截图：全屏和区域截图
  - AT-SPI 无障碍树、元素边界、动作注入
- **MCP stdio 服务器**，供智能体接入
- **Rust SDK**，可嵌入其他项目

### 架构

见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 了解 crate 划分、trait 设计和集成方式。

### 构建

依赖：

- Rust 工具链
- `xvfb`
- `openbox`

Debian/Ubuntu：

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

可执行文件位于 `target/release/linux-x11-sandbox`。

### 运行 MCP 服务器

```bash
./target/release/linux-x11-sandbox
```

服务器通过标准输入输出使用 [MCP](https://modelcontextprotocol.io/) 协议。

### 快速开始

创建一个无头 display 并启动应用：

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"demo","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

创建可见的 Xephyr display（需要宿主机 X display，例如 `DISPLAY=:0`）：

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{"backend":"xephyr"}}}
```

### MCP 工具

| 工具 | 说明 |
|------|------|
| `lxs_display_create` | 创建新的 X11 display（`backend`: `xvfb` 或 `xephyr`） |
| `lxs_display_destroy` | 销毁 display 及其应用 |
| `lxs_display_info` | 获取 display 分辨率与应用数量 |
| `lxs_app_launch` | 在指定 display 上启动应用 |
| `lxs_app_terminate` | 按 PID 终止应用 |
| `lxs_app_list` | 列出 display 上的运行中应用 |
| `lxs_input_click` | 在屏幕坐标点击 |
| `lxs_input_move` | 移动鼠标光标 |
| `lxs_input_scroll` | 滚动指定增量 |
| `lxs_input_type` | 输入文本 |
| `lxs_input_key` | 按下单个键或组合键 |
| `lxs_capture_screenshot` | 全屏截图 |
| `lxs_capture_region` | 区域截图 |
| `lxs_state_window` | 获取当前激活窗口标题 |
| `lxs_state_tree` | 遍历 AT-SPI 无障碍树 |
| `lxs_state_element_bounds` | 获取指定索引元素的边界 |
| `lxs_perform_action` | 对元素执行 AT-SPI 动作 |

### 测试

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

集成测试串行执行，避免 display 编号冲突。

## License

MIT
