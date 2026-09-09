"""One-click registration of skill and MCP server with common agents."""

import json
import os
import shutil
import sys
from pathlib import Path

from linux_x11_sandbox import get_binary_path, get_skill_path


def _log(msg: str) -> None:
    print(f"\033[0;32m[setup]\033[0m {msg}")


def _info(msg: str) -> None:
    print(f"\033[0;34m[info]\033[0m {msg}")


def _warn(msg: str) -> None:
    print(f"\033[1;33m[warn]\033[0m {msg}", file=sys.stderr)


def _link_skill(target_dir: Path) -> None:
    target_dir.mkdir(parents=True, exist_ok=True)
    link = target_dir / "linux-x11-sandbox"
    if link.is_symlink() or link.exists():
        link.unlink()
    link.symlink_to(get_skill_path(), target_is_directory=True)
    _log(f"Linked skill -> {link}")


def _backup(path: Path) -> None:
    backup = path.with_suffix(path.suffix + ".lxs-backup")
    if path.exists() and not backup.exists():
        shutil.copy2(path, backup)
        _log(f"Backed up {path} -> {backup}")


def _update_json_config(path: Path, key: str, value: str) -> None:
    _backup(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    data: dict = {}
    if path.exists():
        try:
            data = json.loads(path.read_text())
        except json.JSONDecodeError:
            _warn(f"{path} is malformed; replacing.")
    data.setdefault("mcpServers", {})
    data["mcpServers"][key] = {"command": value}
    path.write_text(json.dumps(data, indent=2))
    _log(f"Updated {path}")


def _register_skills(project_root: Path) -> None:
    _log("Registering skill for detected agents...")

    home = Path.home()

    if (home / ".pi").exists() or (project_root / ".pi").exists():
        _link_skill(home / ".pi" / "agent" / "skills")
    else:
        _info(
            "pi not detected. To register manually: "
            f"ln -s {get_skill_path()} ~/.pi/agent/skills/linux-x11-sandbox"
        )

    if shutil.which("claude") or (project_root / ".claude").exists():
        _link_skill(project_root / ".claude" / "skills")
    else:
        _info(
            "Claude Code not detected. To register manually: "
            f"ln -s {get_skill_path()} .claude/skills/linux-x11-sandbox"
        )

    if shutil.which("codex") or (home / ".codex").exists():
        _link_skill(home / ".codex" / "skills")
    else:
        _info(
            "Codex not detected. To register manually: "
            f"ln -s {get_skill_path()} ~/.codex/skills/linux-x11-sandbox"
        )

    _link_skill(home / ".agents" / "skills")


def _register_mcp_servers() -> None:
    _log("Registering MCP server for detected agents...")

    binary = str(get_binary_path())
    home = Path.home()

    claude_config = home / ".config" / "claude" / "claude_desktop_config.json"
    if (home / ".config" / "claude").exists() or claude_config.exists():
        _update_json_config(claude_config, "linux-x11-sandbox", binary)
    else:
        _info(f"Claude Desktop not detected. Config path: {claude_config}")

    cursor_config = home / ".cursor" / "mcp.json"
    if (home / ".cursor").exists() or cursor_config.exists():
        _update_json_config(cursor_config, "linux-x11-sandbox", binary)
    else:
        _info(f"Cursor not detected. Config path: {cursor_config}")

    _info(
        f"For other agents, add this MCP server: {{ \"command\": \"{binary}\" }}"
    )


def setup_agents() -> None:
    """Detect common agents and register the skill + MCP server."""
    project_root = Path.cwd()
    _register_skills(project_root)
    _register_mcp_servers()
    print()
    _log("Setup complete. Restart your agent to use linux-x11-sandbox.")
