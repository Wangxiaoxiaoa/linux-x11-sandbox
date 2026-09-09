# linux-x11-sandbox Python package

Python wrapper and distribution package for
[linux-x11-sandbox](https://github.com/Wangxiaoxiaoa/linux-x11-sandbox).

It bundles the Rust MCP server binary and provides a one-click setup command to
register the skill and MCP server with common agents.

## Install

Requires Python 3.10+, Rust toolchain, and Linux.

```bash
pip install linux-x11-sandbox
```

During installation, pip compiles the Rust binary from source and bundles it
into the wheel.

## Use the MCP server

```bash
linux-x11-sandbox
```

The server speaks MCP over stdin/stdout.

## Register with your agent

```bash
linux-x11-sandbox setup
```

This detects installed agents and registers:

- Skill for **pi**, **Claude Code**, **Codex**, and `.agents`.
- MCP server for **Claude Desktop** and **Cursor**.

Restart your agent after setup.

## Python API

```python
from linux_x11_sandbox import get_binary_path

print(get_binary_path())
```

For the full Rust SDK, see the project repository.
