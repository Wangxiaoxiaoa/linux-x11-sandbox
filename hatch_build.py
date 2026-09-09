"""Hatch build hook: compile the Rust binary and bundle it in the wheel."""

import os
import platform
import shutil
import subprocess
from pathlib import Path

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


def _get_wheel_tag() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "linux":
        if machine in ("x86_64", "amd64"):
            platform_tag = "linux_x86_64"
        elif machine in ("arm64", "aarch64"):
            platform_tag = "linux_aarch64"
        else:
            platform_tag = f"linux_{machine}"
    elif system == "darwin":
        platform_tag = "macosx_11_0_arm64"
    else:
        platform_tag = f"{system}_{machine}"
    return f"py3-none-{platform_tag}"


class CustomBuildHook(BuildHookInterface):
    """Build the linux-x11-sandbox Rust binary and copy it into the package."""

    def initialize(self, version: str, build_data: dict) -> None:
        project_root = Path(self.root).resolve()
        binary_name = "linux-x11-sandbox"

        subprocess.run(
            ["cargo", "build", "--release"],
            cwd=str(project_root),
            check=True,
        )

        src = project_root / "target" / "release" / binary_name
        if not src.exists():
            raise RuntimeError(f"Built binary not found: {src}")

        pkg_dir = project_root / "python" / "src" / "linux_x11_sandbox"
        bin_dir = pkg_dir / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        dst = bin_dir / binary_name

        shutil.copy2(src, dst)
        os.chmod(dst, 0o755)

        skill_src = project_root / "skills" / "linux-x11-sandbox"
        if skill_src.exists():
            skill_dst = pkg_dir / "skill"
            if skill_dst.exists():
                shutil.rmtree(skill_dst)
            shutil.copytree(skill_src, skill_dst)

        build_data["tag"] = _get_wheel_tag()
        build_data["pure_python"] = False
