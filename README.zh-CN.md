<div align="center">

# linux-x11-harness

**为 AI 智能体提供隔离的 Linux X11 GUI 运行环境。**

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## 这是什么？

`linux-x11-harness` 可以创建隔离的 X11 显示环境，并通过 MCP 服务器让 AI 智能体控制 GUI 应用。每个显示环境都有独立的 X 服务器、窗口管理器和应用进程组，因此智能体可以点击、输入、截图、读取无障碍树，而不会影响宿主桌面。

典型用途：

- 让智能体操作 GUI 应用，而不接管真实桌面。
- 运行需要真实输入、焦点和截图的端到端测试。
- 捕获无障碍树或屏幕画面用于验证。

## 支持的智能体

一次安装，即可用于任意兼容 MCP 的智能体。`setup` 命令会自动注册到：

- [Claude Code](https://claude.ai/code)
- [Codex](https://github.com/openai/codex)
- [Kimi Code](https://kimi-code.moonshot.cn/)
- [Qwen](https://qwen.aliyun.com/)
- [OpenCode](https://opencode.ai/)
- [Pi](https://pi.ai/)

## 安装

```bash
pip install linux-x11-harness
linux-x11-harness setup
```

环境要求：Python 3.10+、Rust 工具链、已安装 `xvfb` 和 `openbox` 的 Linux 系统。

## 快速开始

启动守护进程：

```bash
linux-x11-harness serve
```

运行 MCP stdio 代理（多数智能体会自动调用）：

```bash
linux-x11-harness mcp
```

其他命令：

```bash
linux-x11-harness status
linux-x11-harness stop
```

## 文档

- [使用指南](docs/usage.md) — 配置显示环境、后端和多智能体隔离。
- [智能体技能](skills/linux-x11-harness/SKILL.md) — 给智能体的快速参考。
- [架构说明](docs/ARCHITECTURE.md) — crate 结构和设计说明。

## 许可证

MIT
