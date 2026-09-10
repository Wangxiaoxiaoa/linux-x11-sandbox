# linux-x11-sandbox Architecture

> **Implementation status:** This document describes the target architecture. The current MVP implements the MCP stdio server, native X11/XTest driver, Xvfb + openbox runtime, and AT-SPI state access. Items marked as reserved or future (CuaDriver adapter, HTTP/SSE, C FFI, runtime submodules, Xephyr backend) are not yet implemented.

## 1. Overview

`linux-x11-sandbox` is a Rust-based Linux GUI automation sandbox platform. It creates and manages multiple isolated X11 display environments, each capable of running GUI applications and exposing built-in automation primitives (mouse, keyboard, screenshot, AT-SPI state).

The system is designed to be consumed in three ways:

1. **MCP Server** — integrated by AI agents (Codex, Claude, DeepSeek Harness, etc.) via the Model Context Protocol.
2. **Rust SDK** — embedded directly into Rust applications as a library.
3. **Plugin / External Sandbox Integration** — embedded or orchestrated by other sandbox platforms (e2b, Daytona, Docker, custom container runtimes) through standard process, C FFI, or network interfaces.

## 2. Design Goals

| Goal | Description |
|------|-------------|
| **Multi-display** | Support multiple concurrent X11 displays (`:99`, `:100`, `:101`, ...). |
| **Backend variety** | Headless (`Xvfb`) X server; `Xephyr` is reserved for future use. |
| **Pluggable driver** | Built-in native driver by default; optional `cua-driver` adapter through a unified `Driver` trait. |
| **Built-in automation** | Mouse, keyboard, screenshot, and AT-SPI state access per display. |
| **No host interference** | Operations on sandbox displays must not steal focus from or affect the user's main desktop. |
| **Three consumption modes** | MCP, SDK, and plugin/embeddable. |
| **External-sandbox friendly** | Runnable inside containers, manageable via standard protocols, configurable through files/env vars. |
| **Clean lifecycle** | Robust startup, health checking, graceful shutdown, and resource cleanup. |

## 3. Three Access Modes

### 3.1 MCP Server

The primary integration path for AI agents. `linux-x11-sandbox` exposes a set of MCP tools prefixed with `lxs_`.

Supported transports:

- `stdio` — local agent integration (default).
- `sse` / HTTP — remote/cloud integration.

Example agent configuration:

```json
{
  "mcpServers": {
    "linux-x11-sandbox": {
      "command": "linux-x11-sandbox",
      "args": ["mcp", "--transport", "stdio"]
    }
  }
}
```

### 3.2 Rust SDK

A programmatic Rust API for direct function calls within Rust applications.

```rust
use linux_x11_sandbox::{Runtime, DisplayConfig, Backend, DriverType};

let runtime = Runtime::new(Default::default()).await?;
let display = runtime
    .create_display(DisplayConfig {
        backend: Backend::Xvfb,
        width: 1280,
        height: 800,
        depth: 24,
        driver: DriverType::Native,
    })
    .await?;

display.launch_app("chromium", &["--no-sandbox"]).await?;
display.driver().click(100, 200, MouseButton::Left, 1).await?;
let png = display.driver().screenshot().await?;
runtime.destroy_display(display.id()).await?;
```

### 3.3 Plugin / Embeddable

`linux-x11-sandbox` can be embedded or orchestrated by other sandbox platforms through:

1. **Rust crate dependency** — consume the SDK.
2. **C FFI** — load as a dynamic library from C/C++/Go/Python/etc.
3. **Subprocess + MCP/HTTP** — spawned and controlled by external orchestrators.
4. **Docker image** — deployed as a container service.

No platform-specific adapter is required. External sandboxes interact through standard interfaces.

## 4. Workspace & Crate Structure

```text
linux-x11-sandbox/
├── Cargo.toml                  # workspace root
├── docs/                       # design and usage documentation
├── .github/                    # CI workflows
├── lxs/
│   ├── core/                   # types, errors, config, Driver trait
│   ├── action/                 # native input backend (XTest / XI2)
│   ├── state/                  # native state backends (capture + a11y)
│   ├── driver/                 # Driver implementations (native + optional cua)
│   ├── runtime/                # runtime engine (display, xserver, wm, process)
│   └── mcp/                    # MCP server + binary entry
└── lxs/mcp/tests/              # MCP integration tests
```

### 4.1 Crate Responsibilities

| Crate | Responsibility | Dependencies |
|-------|----------------|--------------|
| `lxs-core` | Error types, configuration structs, shared data models, and all backend traits. | None |
| `lxs-action` | Native input primitives: mouse move, click, scroll, keyboard type/key. | `lxs-core` |
| `lxs-state` | Native state primitives: screenshot, window state, AT-SPI tree, element bounds, actions. | `lxs-core` |
| `lxs-driver` | Implements the `Driver` trait: `NativeDriver` (combines action + state) and optional `CuaDriver` adapter. | `lxs-core`, `lxs-action`, `lxs-state` |
| `lxs-runtime` | Orchestrates displays: X server/WM/process lifecycle, display allocation, driver injection. | `lxs-core`, `lxs-driver` |
| `lxs-mcp` | MCP server, tool routing, transport adapters, CLI binary. | `lxs-core`, `lxs-runtime` |

### 4.2 Dependency Direction

```text
lxs-core
  ├── lxs-action
  ├── lxs-state
  ├── lxs-driver  (depends on action + state, implements Driver trait)
  └── lxs-runtime (depends on driver, orchestrates everything)
        └── lxs-mcp
```

No reverse dependencies.

### 4.3 AT-SPI Runtime Alignment

Following cua-driver's practice, launched applications inherit accessibility-enabling environment variables (`ACCESSIBILITY_ENABLED=1`, `NO_AT_BRIDGE=0`, `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`, `QT_ACCESSIBILITY=1`). Chromium-family browsers automatically receive `--force-renderer-accessibility`. This matches cua-driver's handling of different toolkits/runtimes so that AT-SPI trees are populated without requiring global screen-reader settings.

## 5. Core Abstractions

### 5.1 `Driver` Trait

The central automation abstraction, defined in `lxs-core`.

```rust
#[async_trait]
pub trait Driver: Send + Sync {
    // ---- input ----
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError>;
    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError>;
    async fn type_text(&self, text: &str) -> Result<(), LxsError>;
    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError>;

    // ---- capture ----
    async fn screenshot(&self) -> Result<Screenshot, LxsError>;
    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError>;

    // ---- a11y ----
    async fn window_state(&self) -> Result<WindowState, LxsError>;
    async fn accessibility_tree(&self, pid: Option<u32>) -> Result<AccessibilityTree, LxsError>;
    async fn element_bounds(&self, pid: u32, index: usize) -> Result<Bounds, LxsError>;
    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError>;
}
```

### 5.2 Native Driver

```rust
pub struct NativeDriver {
    display: String,
    input: Box<dyn InputBackend>,
    capture: Box<dyn CaptureBackend>,
    a11y: Box<dyn A11yBackend>,
}
```

`NativeDriver` is the default implementation. It composes backends from `lxs-action` and `lxs-state`.

### 5.3 Cua-Driver Adapter (Reserved)

```rust
pub struct CuaDriver {
    display: String,
    client: CuaClient,
}
```

`CuaDriver` implements the same `Driver` trait by forwarding calls to an external `cua-driver` instance. This allows users to choose between the lightweight native driver and the full cua-driver backend.

### 5.4 Backend Traits

Defined in `lxs-core`, implemented by `lxs-action` and `lxs-state`:

```rust
#[async_trait]
pub trait InputBackend: Send + Sync {
    async fn click(&self, x: i32, y: i32, button: MouseButton, count: u32) -> Result<(), LxsError>;
    async fn move_mouse(&self, x: i32, y: i32) -> Result<(), LxsError>;
    async fn scroll(&self, dx: i32, dy: i32) -> Result<(), LxsError>;
    async fn type_text(&self, text: &str) -> Result<(), LxsError>;
    async fn key(&self, key: &str, modifiers: &[&str]) -> Result<(), LxsError>;
}

#[async_trait]
pub trait CaptureBackend: Send + Sync {
    async fn screenshot(&self) -> Result<Screenshot, LxsError>;
    async fn screenshot_region(&self, region: Rect) -> Result<Screenshot, LxsError>;
}

#[async_trait]
pub trait A11yBackend: Send + Sync {
    async fn window_state(&self) -> Result<WindowState, LxsError>;
    async fn accessibility_tree(&self, pid: Option<u32>) -> Result<AccessibilityTree, LxsError>;
    async fn element_bounds(&self, pid: u32, index: usize) -> Result<Bounds, LxsError>;
    async fn perform_action(&self, pid: u32, index: usize, action: &str) -> Result<(), LxsError>;
}
```

### 5.5 `Runtime`

Top-level orchestrator.

```rust
pub struct Runtime {
    config: RuntimeConfig,
    displays: DashMap<String, Arc<Display>>,
    allocator: DisplayAllocator,
    supervisor: ProcessSupervisor,
}
```

### 5.6 `Display`

A single isolated X11 environment.

```rust
pub struct Display {
    id: String,
    display: String,
    backend: Backend,
    xserver: ManagedProcess,
    wm: ManagedProcess,
    apps: Mutex<Vec<ManagedProcess>>,
    driver: Arc<dyn Driver>,
    state: AtomicDisplayState,
}
```

`Display` does not depend on a specific driver implementation; it holds `Arc<dyn Driver>`.

## 6. Runtime Internals

`lxs-runtime` contains the infrastructure for managing displays.

```text
lxs-runtime/
└── src/
    ├── lib.rs
    ├── runtime.rs          # Runtime top-level API
    ├── display.rs          # Display struct and lifecycle
    ├── manager.rs          # DisplayManager
    ├── allocator.rs        # Display number allocation
    ├── state.rs            # DisplayState enum
    ├── xserver/
    │   ├── mod.rs          # XServerBackend re-exports + common logic
    │   ├── xvfb.rs         # XvfbBackend implementation
    │   ├── xephyr.rs       # XephyrBackend implementation
    │   └── readiness.rs    # X server readiness detection
    ├── wm/
    │   ├── mod.rs
    │   └── openbox.rs      # Openbox launcher
    ├── process/
    │   ├── mod.rs
    │   ├── spawner.rs      # Child process spawning
    │   ├── managed.rs      # ManagedProcess wrapper
    │   └── supervisor.rs   # Process health monitoring
    └── driver/
        └── mod.rs          # Per-display Driver handle / factory
```

### 6.1 X Server Management

X server backends (`XvfbBackend`, `XephyrBackend`) are implemented in `lxs-runtime`. The `XServerBackend` trait is defined in `lxs-core`.

### 6.2 Window Manager

Window manager launchers (e.g., `OpenboxWM`) live in `lxs-runtime/src/wm/`.

### 6.3 Process Supervision

`ProcessSupervisor` is a background task that polls child processes and triggers cleanup on unexpected exit.

## 7. MCP Tool Reference

All tools are prefixed with `lxs_`.

| Tool | Purpose | Required args |
|------|---------|---------------|
| `lxs_display_create` | Create display (`backend`: `xvfb` or `xephyr`) | — |
| `lxs_display_destroy` | Destroy display | `display_id` |
| `lxs_display_info` | Resolution and app count | `display_id` |
| `lxs_app_launch` | Launch an application | `display_id`, `command` |
| `lxs_app_terminate` | Terminate by PID | `display_id`, `pid` |
| `lxs_input_click` | Click at `(x, y)` with optional button/count | `display_id`, `x`, `y`, `button`, `count` |
| `lxs_input_move` | Move cursor | `display_id`, `x`, `y` |
| `lxs_input_scroll` | Scroll | `display_id` |
| `lxs_input_drag` | Drag from `(x1, y1)` to `(x2, y2)` | `display_id`, `x1`, `y1`, `x2`, `y2` |
| `lxs_input_get_cursor_position` | Get current mouse position | `display_id` |
| `lxs_input_type` | Type text | `display_id`, `text` |
| `lxs_input_key` | Press key or combo | `display_id`, `key` |
| `lxs_capture_screenshot` | Full screenshot | `display_id` |
| `lxs_capture_window` | Screenshot a specific window | `display_id`, `window_id` |
| `lxs_window_focus` | Focus window by id | `display_id`, `window_id` |
| `lxs_window_set_frame` | Set window position and size | `display_id`, `window_id`, `x`, `y`, `width`, `height` |
| `lxs_window_close` | Close window by id | `display_id`, `window_id` |
| `lxs_clipboard_get` | Get clipboard text | `display_id` |
| `lxs_clipboard_set` | Set clipboard text | `display_id`, `text` |
| `lxs_get_desktop_overview` | Desktop overview: processes and windows | `display_id` |
| `lxs_get_window_state` | Window metadata + optional tree + optional screenshot | `display_id`, `pid`, `window_id` |
| `lxs_set_value` | Set AT-SPI editable element value | `display_id`, `pid`, `index`, `value` |
| `lxs_click_element` | Click an AT-SPI element by pid and index | `display_id`, `pid`, `index`, `button` |
| `lxs_wait` | Wait for milliseconds | `ms` |


## 8. SDK API Surface

The SDK exposes async Rust APIs mirroring the MCP tools.

```rust
impl Runtime {
    pub async fn new(config: RuntimeConfig) -> Result<Self, LxsError>;
    pub async fn create_display(&self, config: DisplayConfig) -> Result<DisplayHandle, LxsError>;
    pub async fn destroy_display(&self, id: &str) -> Result<(), LxsError>;
    pub async fn list_displays(&self) -> Vec<DisplayInfo>;
}

impl DisplayHandle {
    pub fn id(&self) -> &str;
    pub fn display(&self) -> &str;
    pub async fn launch_app(&self, command: &str, args: &[&str]) -> Result<AppHandle, LxsError>;
    pub async fn terminate_app(&self, pid: u32) -> Result<(), LxsError>;
    pub fn driver(&self) -> Arc<dyn Driver>;
}
```

## 9. Plugin / External Sandbox Integration

### 9.1 Rust Crate Dependency

External Rust projects add the crate without the MCP binary:

```toml
[dependencies]
linux-x11-sandbox = { version = "0.1", default-features = false }
```

### 9.2 C FFI

A stable C ABI for loading as a dynamic library:

```c
lxs_runtime_t* lxs_runtime_new(const lxs_config_t* config);
lxs_display_t* lxs_display_create(lxs_runtime_t* rt, const lxs_display_config_t* cfg);
int lxs_display_click(lxs_display_t* d, int x, int y);
void lxs_display_destroy(lxs_display_t* d);
void lxs_runtime_free(lxs_runtime_t* rt);
```

### 9.3 Subprocess + MCP/HTTP

External sandboxes spawn `linux-x11-sandbox serve` and control it via MCP or HTTP.

```bash
docker run -p 8080:8080 linux-x11-sandbox:latest serve --transport sse --port 8080
```

```bash
linux-x11-sandbox serve --transport mcp-stdio
```

No custom adapter is required on either side.

## 10. Data Flow Examples

### 10.1 Agent Opens WeChat via MCP

```text
agent
  └─► lxs_display_create({ backend: "xvfb", driver: "native" })
       └─► Runtime creates Display :99
            ├─► starts Xvfb on :99
            ├─► starts openbox on :99
            └─► injects NativeDriver for :99
       ◄── returns { display_id: "d1", display: ":99" }

  └─► lxs_app_launch({ display_id: "d1", command: "wechat" })
       └─► Display :99 launches wechat

  └─► lxs_get_window_state({ display_id: "d1", pid: 1234, window_id: 12345678, include_tree: true })
       └─► NativeDriver queries AT-SPI on :99
       ◄── returns window metadata + tree

  └─► lxs_input_click({ display_id: "d1", x: 100, y: 200 })
       └─► NativeDriver injects XTest click on :99

  └─► lxs_display_destroy({ display_id: "d1" })
       └─► Runtime kills Xvfb, WM, wechat; cleans lock files
```

### 10.2 Rust SDK Direct Call

```text
Rust app
  └─► Runtime::new()
       └─► create_display()
            └─► Display :100 with NativeDriver
       └─► display.driver().click(x, y)
       └─► display.driver().screenshot()
       └─► runtime.destroy_display(id)
```

### 10.3 External Sandbox Spawns Container

```text
e2b sandbox
  └─► docker run linux-x11-sandbox serve --transport sse
       └─► SSE endpoint exposed
            ├─► /health
            ├─► /mcp/v1/tools/list
            └─► /mcp/v1/tools/call
  └─► e2b calls MCP tools over HTTP
       └─► operates displays inside the container
```

## 11. Configuration

Configuration sources, in order of precedence:

1. CLI arguments
2. Environment variables (`LXS_*`)
3. Configuration file (`~/.config/linux-x11-sandbox/config.toml`)
4. Defaults

```toml
[runtime]
display_base = 99
max_displays = 32
default_width = 1280
default_height = 800
default_depth = 24
default_backend = "xvfb"
default_driver = "native"
cleanup_on_exit = true

[mcp]
transport = "stdio"  # "stdio" | "sse"
port = 8080
host = "127.0.0.1"

[xserver]
xvfb_binary = "Xvfb"
xephyr_binary = "Xephyr"
window_manager = "openbox"

[driver]
input_backend = "xtest"  # "xtest" | "xi2"
capture_format = "png"   # "png" | "jpeg"
```

## 12. Lifecycle Management

### 12.1 Startup

1. Parse configuration.
2. Initialize `Runtime` and `DisplayManager`.
3. If MCP mode, start MCP server.
4. Wait for requests.

### 12.2 Display Creation

1. Allocate display number.
2. Start X server process (Xvfb or Xephyr).
3. Wait for `DISPLAY` readiness.
4. Start window manager.
5. Instantiate selected driver (`NativeDriver` or `CuaDriver`).
6. Mark display as `Running`.

### 12.3 Display Destruction

1. Terminate applications.
2. Terminate window manager.
3. Terminate X server.
4. Remove lock files (`/tmp/.X99-lock`, `/tmp/.X11-unix/X99`).
5. Free display number.

### 12.4 Graceful Shutdown

On `SIGTERM` / `SIGINT`:

1. Stop accepting new requests.
2. Destroy all active displays.
3. Exit.

## 13. Error Handling & Observability

- Unified `LxsError` type with structured variants.
- `tracing` for structured logging.
- Per-display health status.
- Prometheus-style metrics (optional): active displays, operations per second, failures.
- `/health` HTTP endpoint for container orchestrators.

## 14. Deployment Patterns

| Pattern | Command / Usage |
|---------|-----------------|
| Local MCP stdio | `linux-x11-sandbox mcp` |
| Remote MCP SSE | `linux-x11-sandbox serve --transport sse --port 8080` |
| Rust SDK | `linux-x11-sandbox = "0.1"` |
| C FFI plugin | `liblinux_x11_sandbox.so` |
| Docker | `docker run -p 8080:8080 linux-x11-sandbox` |
| Inside e2b/Daytona | Spawn container or subprocess with MCP/HTTP |

## 15. Future Considerations

- **Wayland/XWayland support** for future Linux environments.
- **Recording** (video capture of a display).
- **Snapshot/restore** of a display state.
- **Remote VNC** access for debugging visible `Xephyr` displays.
- **Resource quotas** (memory, CPU) per display via cgroup integration.

## 16. Summary

`linux-x11-sandbox` is a focused Linux X11 GUI sandbox with a built-in automation driver and a pluggable driver interface (including a reserved `cua-driver` adapter). It intentionally does not implement general-purpose container sandboxing; instead, it is designed to be composed with existing sandbox platforms through standard interfaces (MCP, HTTP, Rust SDK, C FFI).

Three access modes share the same core runtime:

- **MCP** for agents.
- **SDK** for Rust applications.
- **Plugin/FFI/subprocess** for external sandbox platforms.
