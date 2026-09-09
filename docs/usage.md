# Usage

## Build

```bash
cargo build --release
```

The MCP server binary is `target/release/linux-x11-sandbox`.

## Run the MCP server

```bash
./target/release/linux-x11-sandbox
```

The server speaks MCP over stdio.

## Example MCP session

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"demo","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

## Run tests

```bash
cargo test
```

The MCP integration test lives in `lxs/mcp/tests/mcp_smoke.rs`.
