<div align="center">

# linux-x11-sandbox

**面向自动化测试与 AI 智能体的自包含 Linux X11 GUI 沙盒。**

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## 这是什么？

`linux-x11-sandbox` 在独立的 X11 display 中运行 GUI 应用，并允许智能体通过鼠标、键盘、截图和 AT-SPI 无障碍树与应用交互。

每个沙盒拥有独立的 X 服务器、窗口管理器和应用进程组，不共享窗口、焦点或剪贴板。24 个自动化工具均配有真实效果集成测试，覆盖真实 X11 输入、窗口变化、截图和无障碍操作。

典型用途：

- 让智能体操作 GUI 应用而不接管宿主机桌面。
- 运行需要真实屏幕、输入和窗口焦点的端到端测试。
- 采集截图或无障碍树用于验证。

可作为 **MCP 服务器**、**Rust SDK** 或 **可组合沙盒层**（Docker/VM 内）使用。所有 X11 协议访问均通过 [`x11rb`](https://github.com/psychon/x11rb)；不链接 Xlib。

## 架构

见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 快速安装

```bash
pip install linux-x11-sandbox
linux-x11-sandbox setup
```

依赖：Python 3.10+、Rust 工具链、已安装 `xvfb` 和 `openbox` 的 Linux。

## 手动构建

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

可执行文件：`target/release/linux-x11-sandbox`。

---

## 1. 作为 MCP 服务器

服务器通过标准输入输出使用 JSON-RPC 2.0。

### 启动

```bash
./target/release/linux-x11-sandbox
```

### 工具列表

| 工具 | 说明 |
|------|------|
| `lxs_display_create` | 创建 display（`backend`: `xvfb` 或 `xephyr`） |
| `lxs_display_destroy` | 销毁 display |
| `lxs_display_info` | 分辨率与应用数量 |
| `lxs_app_launch` | 启动应用 |
| `lxs_app_terminate` | 按 PID 终止应用 |
| `lxs_input_click` | 在 `(x, y)` 点击，支持 `button`（`left`/`right`/`middle`）与 `count`（单击/双击/三击） |
| `lxs_input_move` | 移动光标 |
| `lxs_input_scroll` | 滚动 |
| `lxs_input_drag` | 从 `(x1, y1)` 拖动到 `(x2, y2)` |
| `lxs_input_get_cursor_position` | 获取当前鼠标位置 |
| `lxs_input_type` | 输入文本 |
| `lxs_input_key` | 按键或组合键 |
| `lxs_capture_screenshot` | 全屏截图 |
| `lxs_capture_window` | 截取指定窗口 |
| `lxs_window_focus` | 按 `window_id` 聚焦窗口 |
| `lxs_window_set_frame` | 设置窗口位置和大小 |
| `lxs_window_close` | 按 `window_id` 关闭窗口 |
| `lxs_clipboard_get` | 获取剪贴板文本 |
| `lxs_clipboard_set` | 设置剪贴板文本 |
| `lxs_get_desktop_overview` | 桌面概览：进程与窗口 |
| `lxs_get_window_state` | 窗口元数据 + 可选树 + 可选截图 |
| `lxs_set_value` | 设置 AT-SPI 可编辑元素值 |
| `lxs_click_element` | 按 `pid` 和 `index` 点击 AT-SPI 元素 |
| `lxs_wait` | 等待 `ms` 毫秒 |

### 示例会话

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

---

## 2. 作为 Rust SDK

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

## 3. Docker

```bash
docker build -t linux-x11-sandbox .
docker compose up -d
```

使用 `scripts/docker-mcp.sh` 作为 MCP 命令，或运行 `docker exec -i <container> linux-x11-sandbox`。

---

## 测试

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

## 许可证

MIT
