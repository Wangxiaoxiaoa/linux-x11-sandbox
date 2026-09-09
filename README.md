# linux-x11-sandbox

Linux X11 GUI sandbox with a built-in automation driver.

Provides an MCP server, a Rust SDK, and a plugin/embeddable interface for running
GUI applications in isolated X11 displays. Each display gets its own X server
(Xvfb), window manager, and set of applications. Displays do not share windows,
focus, clipboard, or desktop shell—only the underlying filesystem.

## Features

- Headless X11 display per sandbox (Xvfb + openbox)
- Built-in native automation driver
  - Mouse: move, click, scroll
  - Keyboard: type text, key combos
  - Screenshot: full screen and region
  - AT-SPI accessibility tree, element bounds, action injection
- MCP stdio server for agent integration
- Rust SDK for embedding

## Build

Requires:

- Rust toolchain
- `xvfb`
- `openbox`

On Debian/Ubuntu:

```bash
sudo apt-get install -y xvfb openbox
```

Build the MCP server:

```bash
cargo build --release
```

The binary is at `target/release/linux-x11-sandbox`.

## Run the MCP server

```bash
./target/release/linux-x11-sandbox
```

The server speaks [MCP](https://modelcontextprotocol.io/) over stdio.

## Example MCP session

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"demo","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"lxs_display_create","arguments":{}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"lxs_app_launch","arguments":{"display_id":"d-99","command":"xterm","args":[]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"lxs_capture_screenshot","arguments":{"display_id":"d-99"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"lxs_display_destroy","arguments":{"display_id":"d-99"}}}
```

## Tools

| Tool | Description |
|------|-------------|
| `lxs_display_create` | Create a new X11 display |
| `lxs_display_destroy` | Destroy a display and its apps |
| `lxs_display_info` | Get display resolution and app count |
| `lxs_app_launch` | Launch an application on a display |
| `lxs_app_terminate` | Terminate an application by PID |
| `lxs_app_list` | List running applications on a display |
| `lxs_input_click` | Click at screen coordinates |
| `lxs_input_move` | Move the mouse cursor |
| `lxs_input_scroll` | Scroll by a delta |
| `lxs_input_type` | Type text |
| `lxs_input_key` | Press a key or key combination |
| `lxs_capture_screenshot` | Take a screenshot |
| `lxs_capture_region` | Take a screenshot of a region |
| `lxs_state_window` | Get the active window title |
| `lxs_state_tree` | Walk the AT-SPI accessibility tree |
| `lxs_state_element_bounds` | Get bounds of an indexed element |
| `lxs_perform_action` | Perform an AT-SPI action on an element |

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for crate layout, trait design, and integration patterns.

## License

MIT
