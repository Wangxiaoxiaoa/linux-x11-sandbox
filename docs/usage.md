# Usage

## Build

```bash
cargo build --release
```

The MCP server binary is `target/release/linux-x11-harness`.

## Run the MCP server

The MCP server is a long-lived daemon plus a per-agent stdio proxy.

```bash
./target/release/linux-x11-harness serve  # start daemon
./target/release/linux-x11-harness mcp    # stdio proxy (auto-starts daemon)
./target/release/linux-x11-harness status # check daemon
./target/release/linux-x11-harness stop   # stop daemon
```

## Multiple isolated instances

Each daemon owns one set of displays. To give a child agent its own isolated harness, run a dedicated daemon on a unique socket:

```bash
./target/release/linux-x11-harness serve --socket /tmp/lxh-agent-1.sock
./target/release/linux-x11-harness mcp --socket /tmp/lxh-agent-1.sock
./target/release/linux-x11-harness stop --socket /tmp/lxh-agent-1.sock
```

`--socket` overrides the default `/tmp/linux-x11-harness.sock` and the `LXH_SOCKET_PATH` environment variable.

## Example MCP session

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxh_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxh_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxh_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxh_display_destroy","arguments":{"display_id":"d-99"}}}
```

## Run tests

```bash
cargo test -- --test-threads=1
```

Integration tests live in `lxh/daemon/tests/integration.rs`. They exercise the daemon end-to-end and must run sequentially because each test allocates X11 displays.
