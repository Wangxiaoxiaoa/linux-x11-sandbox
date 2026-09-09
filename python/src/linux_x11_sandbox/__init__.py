"""Python wrapper for the linux-x11-sandbox native automation driver."""

from pathlib import Path


def get_binary_path() -> Path:
    """Return the path to the bundled linux-x11-sandbox executable."""
    return Path(__file__).parent / "bin" / "linux-x11-sandbox"


def get_skill_path() -> Path:
    """Return the path to the bundled Agent Skill directory."""
    return Path(__file__).parent / "skill"
