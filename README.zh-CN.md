<div align="center">

# linux-x11-sandbox

内置原生自动化驱动的 Linux X11 GUI 沙盒。

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

`linux-x11-sandbox` 在独立的 X11 display 中运行 GUI 应用。每个沙盒拥有独立的 X 服务器、窗口管理器和应用进程。display 之间不共享窗口、焦点、剪贴板或桌面 shell，只共享底层文件系统。

## 特性

- **每个沙盒一个独立 X11 display** —— 无头 `Xvfb` 或可见 `Xephyr` + `openbox`
- **内置原生自动化驱动**
  - 鼠标：移动、点击、滚动
  - 键盘：输入文本、组合键
  - 截图：全屏和区域截图
  - AT-SPI 无障碍树、元素边界、动作注入
- **MCP stdio 服务器**，供智能体接入
- **Rust SDK**，可嵌入其他项目

## 架构

见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 了解 crate 划分、trait 设计和集成方式。

## 构建

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

## 运行 MCP 服务器

```bash
./target/release/linux-x11-sandbox
```

服务器通过标准输入输出使用 [MCP](https://modelcontextprotocol.io/) 协议。

## 快速开始

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

## MCP 工具

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

## 测试

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
```

集成测试串行执行，避免 display 编号冲突。

## 许可证

MIT
