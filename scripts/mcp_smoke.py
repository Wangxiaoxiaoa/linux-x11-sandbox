#!/usr/bin/env python3
"""Smoke test for the linux-x11-sandbox MCP stdio server."""

import json
import subprocess
import sys


def main() -> int:
    proc = subprocess.Popen(
        ["./target/release/linux-x11-sandbox"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    def call(method: str, params: dict, req_id: int) -> dict:
        msg = (
            json.dumps(
                {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
            )
            + "\n"
        )
        proc.stdin.write(msg)  # type: ignore[union-attr]
        proc.stdin.flush()  # type: ignore[union-attr]
        line = proc.stdout.readline()  # type: ignore[union-attr]
        return json.loads(line)

    init = call(
        "initialize",
        {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "smoke", "version": "0.1.0"},
        },
        1,
    )
    assert init["result"]["serverInfo"]["name"] == "linux-x11-sandbox", init

    tools = call("tools/list", {}, 2)
    tool_names = {t["name"] for t in tools["result"]["tools"]}
    required = {
        "lxs_display_create",
        "lxs_display_destroy",
        "lxs_display_info",
        "lxs_app_launch",
        "lxs_app_terminate",
        "lxs_app_list",
        "lxs_input_click",
        "lxs_input_move",
        "lxs_input_scroll",
        "lxs_input_type",
        "lxs_input_key",
        "lxs_capture_screenshot",
        "lxs_capture_region",
        "lxs_state_window",
        "lxs_state_tree",
        "lxs_state_element_bounds",
        "lxs_perform_action",
    }
    missing = required - tool_names
    assert not missing, f"missing tools: {missing}"

    display = call(
        "tools/call", {"name": "lxs_display_create", "arguments": {}}, 3
    )
    display_id = display["result"]["display_id"]
    assert display["result"]["display"].startswith(":"), display

    info = call(
        "tools/call",
        {"name": "lxs_display_info", "arguments": {"display_id": display_id}},
        4,
    )
    assert info["result"]["width"] > 0, info
    assert info["result"]["height"] > 0, info

    shot = call(
        "tools/call",
        {"name": "lxs_capture_screenshot", "arguments": {"display_id": display_id}},
        5,
    )
    assert shot["result"]["mimeType"] == "image/png", shot
    assert len(shot["result"]["data"]) > 0, shot

    destroyed = call(
        "tools/call",
        {"name": "lxs_display_destroy", "arguments": {"display_id": display_id}},
        6,
    )
    assert destroyed["result"]["success"], destroyed

    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()

    print("mcp smoke test passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
