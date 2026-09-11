#!/usr/bin/env bash
# MCP stdio bridge to a running linux-x11-harness Docker container.
# Use this as the MCP server command in agent configs.
set -euo pipefail

CONTAINER="${LXH_CONTAINER:-linux-x11-harness}"
exec docker exec -i "$CONTAINER" linux-x11-harness "$@"
