<div align="center">

# linux-x11-sandbox

内置原生自动化驱动的 Linux X11 GUI 沙盒。

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## 这是什么？

`linux-x11-sandbox` 是一个**轻量级 Linux GUI 自动化沙盒**。它让 AI 智能体和程序在独立的 X11 display 中运行真实的 GUI 应用（例如浏览器、终端、IDE、办公软件），并通过鼠标、键盘、截图和 AT-SPI 无障碍树与应用交互。

与完整的虚拟机或容器不同，它只关注**显示与输入层**：每个沙盒拥有独立的 X 服务器、窗口管理器和应用进程组。沙盒之间不共享窗口、焦点、剪贴板或桌面 shell，但仍与宿主机共享文件系统。因此它启动快、资源占用低，并且容易与现有隔离技术组合使用。

典型使用场景：

- 让 AI 智能体操作 GUI 应用，而不接管宿主机桌面。
- 运行需要真实屏幕、真实输入和真实窗口焦点的端到端测试。
- 为模型训练或验证采集截图和无障碍树。
- 在同一台机器上运行多个独立的 GUI 会话。

它可以通过三种方式使用：

1. **MCP 服务器** —— 接入任何支持 [Model Context Protocol](https://modelcontextprotocol.io/) 的智能体。
2. **Rust SDK** —— 直接把沙盒和驱动嵌入你的 Rust 项目。
3. **可组合沙盒层** —— 运行在现有容器/VM 沙盒（Docker、e2b、Daytona 等）内部，为其增加 X11 自动化能力。

## 架构

见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 了解 crate 划分、trait 设计和集成方式。

## 快速安装

安装 Python 发行包：

```bash
pip install linux-x11-sandbox
```

然后向智能体注册自带的 skill 和 MCP 服务器：

```bash
linux-x11-sandbox setup
```

重启智能体即可。setup 命令会自动检测 pi、Claude Code、Codex、Claude Desktop、Cursor 和 `.agents`。

依赖：

- Python 3.10+
- Rust 工具链
- 已安装 `xvfb` 和 `openbox` 的 Linux

## 手动构建

依赖：

- Rust 工具链
- `xvfb`
- `openbox`

Debian/Ubuntu：

```bash
sudo apt-get install -y xvfb openbox
cargo build --release
```

MCP 服务器可执行文件位于 `target/release/linux-x11-sandbox`。

---

## 1. 作为 MCP 服务器使用

MCP 服务器通过标准输入输出使用 JSON-RPC 2.0。任何兼容 MCP 的智能体都可以启动它。

### 1.1 启动服务器

通过 pip 安装后：

```bash
linux-x11-sandbox
```

或者直接使用构建好的 Rust 二进制：

```bash
./target/release/linux-x11-sandbox
```

### 1.2 一键注册智能体

```bash
linux-x11-sandbox setup
```

该命令会为检测到的智能体注册 skill 和 MCP 服务器。执行后重启智能体即可。

### 1.3 手动注册

skill 位于 [`skills/linux-x11-sandbox/SKILL.md`](skills/linux-x11-sandbox/SKILL.md)。软链接到智能体的 skill 目录：

- **pi**：`~/.pi/agent/skills/linux-x11-sandbox/`
- **Claude Code**：`.claude/skills/linux-x11-sandbox/`
- **Codex**：`.codex/skills/linux-x11-sandbox/`

MCP 配置大多数智能体使用 `mcpServers`。Claude Desktop 示例：

```json
{
  "mcpServers": {
    "linux-x11-sandbox": {
      "command": "linux-x11-sandbox"
    }
  }
}
```

### 1.4 MCP 工具

连接后，智能体可以调用：

| 工具 | 说明 |
|------|------|
| `lxs_display_create` | 创建 display（`backend`: `xvfb` 或 `xephyr`） |
| `lxs_display_destroy` | 销毁 display 及应用 |
| `lxs_display_info` | 分辨率与应用数量 |
| `lxs_app_launch` | 启动应用 |
| `lxs_app_terminate` | 按 PID 终止应用 |
| `lxs_app_list` | 列出应用 PID |
| `lxs_input_click` | 在 `(x, y)` 点击，支持 `button` 和 `count` |
| `lxs_input_move` | 移动光标 |
| `lxs_input_scroll` | 滚动 |
| `lxs_input_drag` | 从 `(x1, y1)` 拖动到 `(x2, y2)` |
| `lxs_input_get_cursor_position` | 获取当前鼠标位置 |
| `lxs_input_type` | 输入文本 |
| `lxs_input_key` | 按键或组合键 |
| `lxs_capture_screenshot` | 全屏截图 |
| `lxs_capture_region` | 区域截图 |
| `lxs_window_focus` | 聚焦当前激活窗口 |
| `lxs_window_raise` | 将当前激活窗口置顶 |
| `lxs_window_resize` | 调整当前激活窗口大小 |
| `lxs_window_move` | 移动当前激活窗口 |
| `lxs_window_list` | 列出顶层窗口 |
| `lxs_clipboard_get` | 获取剪贴板文本 |
| `lxs_clipboard_set` | 设置剪贴板文本 |
| `lxs_wait` | 等待 `ms` 毫秒 |
| `lxs_state_window` | 当前激活窗口标题 |
| `lxs_state_tree` | AT-SPI 无障碍树（含 role/name/value/states/actions） |
| `lxs_state_element_bounds` | 元素边界 |
| `lxs_perform_action` | 执行 AT-SPI 动作 |
| `lxs_state_focus_element` | 聚焦元素但不置顶窗口 |
| `lxs_state_scroll_element` | 滚动元素到可见区域 |
| `lxs_state_set_value` | 设置元素值 |
| `lxs_state_type_into_editable` | 在可编辑元素中输入文本 |
| `lxs_state_find_element` | 按 role/name/value 子串查找元素 |

### 1.5 示例会话

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

---

## 2. 作为 Rust SDK 使用

在 `Cargo.toml` 中引入需要的 crate：

```toml
[dependencies]
lxs-core = { path = "path/to/linux-x11-sandbox/lxs/core" }
lxs-runtime = { path = "path/to/linux-x11-sandbox/lxs/runtime" }
lxs-driver = { path = "path/to/linux-x11-sandbox/lxs/driver" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

示例：

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

`lxs-core` 中的 `Driver` trait 允许替换后端（例如未来可能提供的 `CuaDriver` 适配器）。

---

## 3. 作为可组合沙盒层使用

`linux-x11-sandbox` **不会**替代通用容器或 VM 沙盒。相反，它作为一层 X11 自动化能力运行在这些沙盒内部。

典型部署方式：

```
Docker / e2b / Daytona / VM
└── linux-x11-sandbox
    ├── Xvfb 或 Xephyr
    ├── openbox
    └── 目标应用
```

这样文件系统/网络隔离仍由外层沙盒负责，而智能体获得一个可交互的真实 GUI 环境。

### 3.1 Docker

构建镜像：

```bash
docker build -t linux-x11-sandbox .
```

启动持久化容器：

```bash
docker compose up -d
```

在智能体 MCP 配置中使用自带的桥接脚本作为服务器命令：

```json
{
  "mcpServers": {
    "linux-x11-sandbox": {
      "command": "/path/to/linux-x11-sandbox/scripts/docker-mcp.sh"
    }
  }
}
```

或者通过 `docker exec` 直接调用：

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | docker exec -i linux-x11-sandbox linux-x11-sandbox
```

### 3.2 一次性容器

```bash
docker run -i --rm linux-x11-sandbox
```

然后通过标准输入发送 MCP JSON-RPC。

---

## 测试

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

集成测试串行执行，避免 display 编号冲突。

## 许可证

MIT
