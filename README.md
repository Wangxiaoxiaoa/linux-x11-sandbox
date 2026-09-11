<div align="center">

# linux-x11-harness

**Run Linux GUI apps in isolated X11 displays for AI agents.**

[![CI](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml/badge.svg)](https://github.com/Wangxiaoxiaoa/linux-x11-harness/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) · [中文](README.zh-CN.md)

</div>

---

## What is this?

`linux-x11-harness` creates isolated X11 displays and lets AI agents control GUI applications through an MCP server. Each display gets its own X server, window manager, and application process group, so agents can click, type, capture screenshots, and read accessibility trees without touching the host desktop.

Typical uses:

- Let an agent operate a GUI app without taking over your real desktop.
- Run end-to-end tests that need real input, focus, and screenshots.
- Capture accessibility trees or screen recordings for verification.

## Supported agents

Install once and use with any MCP-compatible agent. The setup command registers the server for:

- [Claude Code](https://claude.ai/code)
- [Codex](https://github.com/openai/codex)
- [Kimi Code](https://kimi-code.moonshot.cn/)
- [Qwen](https://qwen.aliyun.com/)
- [OpenCode](https://opencode.ai/)
- [Pi](https://pi.ai/)

## Install

```bash
pip install linux-x11-harness
linux-x11-harness setup
```

Requirements: Python 3.10+, Rust toolchain, Linux with `xvfb` and `openbox`.

## Quick start

Start the daemon:

```bash
linux-x11-harness serve
```

Run the MCP stdio proxy (most agents call this automatically):

```bash
linux-x11-harness mcp
```

Other commands:

```bash
linux-x11-harness status
linux-x11-harness stop
```

## Documentation

- [Usage guide](docs/usage.md) — configure displays, backends, and multi-agent isolation.
- [Agent skill](skills/linux-x11-harness/SKILL.md) — quick reference for agents.
- [Architecture](docs/ARCHITECTURE.md) — crate layout and design notes.

## License

MIT
