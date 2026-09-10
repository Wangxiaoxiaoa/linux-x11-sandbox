#!/usr/bin/env bash
# MCP stdio bridge to a running linux-x11-sandbox Docker container.
# Use this as the MCP server command in agent configs.
set -euo pipefail

CONTAINER="${LXS_CONTAINER:-linux-x11-sandbox}"
exec docker exec -i "$CONTAINER" linux-x11-sandbox "$@"
