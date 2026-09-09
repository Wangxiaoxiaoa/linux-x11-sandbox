"""CLI entry point: linux-x11-sandbox [mcp|setup]."""

import argparse
import subprocess
import sys

from linux_x11_sandbox import get_binary_path
from linux_x11_sandbox._install import setup_agents


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="linux-x11-sandbox",
        description="Linux X11 GUI sandbox with native automation driver.",
    )
    sub = parser.add_subparsers(dest="command")

    sub.add_parser("mcp", help="Run the MCP stdio server (default)")
    sub.add_parser("setup", help="Register skill and MCP server with detected agents")

    args = parser.parse_args()

    if args.command == "setup":
        setup_agents()
    else:
        binary = get_binary_path()
        if not binary.exists():
            print(
                f"Binary not found: {binary}. "
                "Reinstall the package or run 'cargo build --release'.",
                file=sys.stderr,
            )
            sys.exit(1)
        subprocess.run([str(binary)], stdin=sys.stdin, stdout=sys.stdout, stderr=sys.stderr)


if __name__ == "__main__":
    main()
