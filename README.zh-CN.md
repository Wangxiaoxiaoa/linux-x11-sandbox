<div align="center">

# linux-x11-harness

**面向自动化测试与 AI 智能体的自包含 Linux X11 GUI 沙盒。**

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## 这是什么？

`linux-x11-harness` 在独立的 X11 display 中运行 GUI 应用，并允许智能体通过鼠标、键盘、截图和 AT-SPI 无障碍树与应用交互。

每个沙盒拥有独立的 X 服务器、窗口管理器和应用进程组，不共享窗口、焦点或剪贴板。24 个自动化工具均配有真实效果集成测试，覆盖真实 X11 输入、窗口变化、截图和无障碍操作。

典型用途：

- 让智能体操作 GUI 应用而不接管宿主机桌面。
- 运行需要真实屏幕、输入和窗口焦点的端到端测试。
- 采集截图或无障碍树用于验证。

可作为 **MCP 服务器**、**Rust SDK** 或 **可组合沙盒层**（Docker/VM 内）使用。

## 特性

| 特性 | 说明 |
| --- | --- |
| **独立 display** | 每个沙盒都是独立的 `DISPLAY`，拥有独立的 X 服务器和窗口管理器。 |
| **24 个自动化工具** | 覆盖鼠标、键盘、窗口、元素、截图、剪贴板、状态和生命周期。 |
| **MCP 服务器** | 守护进程 + stdio 代理；接入任意 MCP 兼容智能体。 |
| **Rust SDK** | 在 Rust 中直接组合 `Runtime`、`Display` 与 `Driver`。 |
| **Docker 支持** | 提供开箱即用的 `Dockerfile` 与 `docker-compose.yml`。 |
| **无头或有头** | CI 用 `xvfb`，可视化调试用 `xephyr`。 |

## 架构

见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 了解 crate 布局与设计目标。

## 快速安装

```bash
pip install linux-x11-harness
linux-x11-harness setup
```

依赖：Python 3.10+、Rust 工具链、已安装 `xvfb` 和 `openbox` 的 Linux。

## 手动构建

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

二进制：`target/release/linux-x11-harness`。

---

## 作为 MCP 服务器使用

`linux-x11-harness serve` 守护进程持有 display 与驱动；每个智能体运行轻量的 `linux-x11-harness mcp` stdio 代理，通过 Unix socket 转发 JSON-RPC。

### 启动

```bash
./target/release/linux-x11-harness serve   # 启动守护进程
./target/release/linux-x11-harness mcp     # stdio 代理（自动启动守护进程）
./target/release/linux-x11-harness status  # 检查守护进程
./target/release/linux-x11-harness stop    # 停止守护进程
```

### 工具分类

#### Display 与应用生命周期

| 工具 | 说明 |
| --- | --- |
| `lxh_display_create` | 创建 display（`backend`：`xvfb` 或 `xephyr`，`persistent`：布尔值） |
| `lxh_display_attach` | 附加到已有 display（如 `:0`） |
| `lxh_display_detach` | 从已有 display 分离，不销毁 |
| `lxh_display_destroy` | 销毁 display |
| `lxh_display_info` | 分辨率与应用数量 |
| `lxh_app_launch` | 启动应用 |
| `lxh_app_terminate` | 按 PID 终止应用 |
| `lxh_wait` | 等待指定毫秒 |

#### 输入

| 工具 | 说明 |
| --- | --- |
| `lxh_input_click` | 在 `(x, y)` 点击，支持 `button`（`left`/`right`/`middle`）与 `count` |
| `lxh_input_move` | 移动光标 |
| `lxh_input_scroll` | 滚动 |
| `lxh_input_drag` | 从 `(x1, y1)` 拖动到 `(x2, y2)` |
| `lxh_input_get_cursor_position` | 获取当前鼠标位置 |
| `lxh_input_type` | 输入文本 |
| `lxh_input_key` | 按键或组合键 |

#### 窗口管理

| 工具 | 说明 |
| --- | --- |
| `lxh_window_focus` | 按 `window_id` 聚焦窗口 |
| `lxh_window_set_frame` | 设置窗口位置与大小 |
| `lxh_window_close` | 按 `window_id` 关闭窗口 |

#### 截图、剪贴板与状态

| 工具 | 说明 |
| --- | --- |
| `lxh_capture_screenshot` | 全屏截图 |
| `lxh_capture_window` | 指定窗口截图 |
| `lxh_clipboard_get` | 获取剪贴板文本 |
| `lxh_clipboard_set` | 设置剪贴板文本 |
| `lxh_get_desktop_overview` | 桌面概览：进程与窗口 |
| `lxh_get_window_state` | 窗口元数据 + 可选无障碍树 + 可选截图 |

#### 无障碍操作

| 工具 | 说明 |
| --- | --- |
| `lxh_set_value` | 设置 AT-SPI 可编辑元素的值 |
| `lxh_click_element` | 按 `pid` 与 `index` 点击 AT-SPI 元素 |

### 示例会话

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxh_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxh_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxh_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxh_display_destroy","arguments":{"display_id":"d-99"}}}
```

---

## 作为 Rust SDK 使用

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

MCP 命令可使用 `scripts/docker-mcp.sh`，或运行 `docker exec -i <container> linux-x11-harness`。

---

## 测试

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

## 许可证

MIT
