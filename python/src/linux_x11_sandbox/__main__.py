"""CLI entry point: linux-x11-sandbox [mcp|serve|status|stop|setup]."""

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

    sub.add_parser("mcp", help="Run the MCP stdio proxy (default)")
    sub.add_parser("serve", help="Start the daemon")
    sub.add_parser("status", help="Check daemon status")
    sub.add_parser("stop", help="Stop the daemon")
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
        cmd = [str(binary)]
        if args.command:
            cmd.append(args.command)
        subprocess.run(cmd, stdin=sys.stdin, stdout=sys.stdout, stderr=sys.stderr)


if __name__ == "__main__":
    main()
