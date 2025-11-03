# Accessibility MCP Server — Architecture and Design

## Overview

This crate (`accessibility_mcp`) provides a **Model Context Protocol (MCP) server** that exposes
a live view of an application’s accessibility tree to coding agents or developer tools.

The goal is to enable **automated testing, analysis, and feedback** of accessibility features
in native applications, without requiring any changes to how accessibility is implemented.

When used in an app, developers simply call:

```rust
let _mcp = accessibility_mcp::start_mcp_server();
```

This launches a lightweight MCP server within the app that provides full access to that app’s
accessibility tree and actions, using the same interfaces as real assistive technologies.

---

## Key Principles

1. **Consumer-Only Design**
   The MCP server consumes accessibility data **via system APIs**, not by reading app internals.

   * It works regardless of whether the app uses [AccessKit], native platform APIs, or manual accessibility implementation.

2. **App-Scoped Access**
   The server only exposes accessibility data for the **current process**.

3. **Cross-Platform Operation**
   Supported platforms:

   * macOS (AXAPI)
   * Windows (UI Automation)
   * Linux (AT-SPI2)

4. **Minimal Intrusion**
   The server can be toggled by a feature flag and requires no extra permissions
   when inspecting the local app.

---

## Data Flow

```
+---------------------------+
| App (Egui / Bevy / etc.)  |
+-------------+-------------+
              |
              | Accessibility updates
              v
+---------------------------+
| System Accessibility API  |
|  (AXAPI / UIA / AT-SPI)   |
+-------------+-------------+
              |
              | Queries and actions
              v
+---------------------------+
| Platform Consumer Backend |
+-------------+-------------+
              |
              | Normalized to AccessKit-like model
              v
+---------------------------+
| MCP Server                |
|  (JSON-RPC over stdio)    |
+-------------+-------------+
              |
              | Commands / responses
              v
+---------------------------+
| Coding Agent / IDE Plugin |
+---------------------------+
```

---

## Crate Layout

```
accessibility_mcp/
├─ src/
│  ├─ lib.rs
│  ├─ server.rs              # Core MCP server loop
│  ├─ protocol.rs            # MCP request/response schema
│  ├─ platform/
│  │  ├─ mod.rs
│  │  ├─ macos.rs
│  │  ├─ windows.rs
│  │  └─ linux.rs
│  ├─ consumer.rs            # Optional AccessKit normalization layer
│  └─ util.rs
├─ examples/
│  └─ egui_app.rs
└─ ARCHITECTURE.md
```

---

## Public API

### `start_mcp_server`

```rust
/// Starts an MCP server that exposes the accessibility tree
/// for the current process.
///
/// This function spawns a background task that listens for
/// MCP requests over stdio or a local socket.
///
/// Returns a handle that can be used to stop the server.
pub fn start_mcp_server(config: Option<Config>) -> anyhow::Result<McpHandle>;
```

#### `Config`

```rust
pub struct Config {
    pub transport: TransportKind, // e.g., stdio, unix socket, tcp
    pub port: Option<u16>,         // used if TransportKind::Tcp
    pub normalize: bool,           // whether to normalize via AccessKit model
    pub log_level: LogLevel,
}
```

#### `McpHandle`

```rust
pub struct McpHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}
```

---

## Platform Consumer Backends

All platform backends implement the following trait:

```rust
pub trait AccessibilityProvider: Send + Sync {
    fn get_root(&self) -> Result<Node>;
    fn get_children(&self, node_id: NodeId) -> Result<Vec<Node>>;
    fn get_node(&self, node_id: NodeId) -> Result<Node>;
    fn perform_action(&self, node_id: NodeId, action: Action) -> Result<()>;
}
```

### macOS Backend

* API: **AXAPI** (`AXUIElementRef`)
* Crate dependencies: `core-foundation`, `objc2`, `accesskit_macos` (optional)
* Implementation details:

  * Use `AXUIElementCreateApplication(getpid())` as the root.
  * Wrap common queries via `AXUIElementCopyAttributeValue`.
  * Convert to unified `Node` model.

### Windows Backend

* API: **UI Automation** (COM)
* Crate dependencies: `windows`
* Implementation details:

  * Create `IUIAutomation` via `CoCreateInstance`.
  * Use `ElementFromHandle` with app’s main HWND.
  * Enumerate children via `FindAll`.

### Linux Backend

* API: **AT-SPI2 (DBus)**
* Crate dependencies: `zbus`, `atspi`
* Implementation details:

  * Connect to the session bus.
  * Query `/org/a11y/atspi/accessible/<pid>` root.
  * Translate AT-SPI attributes and actions to `Node`.

---

## Node and Action Model

All nodes are normalized to a common schema (roughly AccessKit’s data model):

```rust
pub struct Node {
    pub id: NodeId,
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub bounds: Option<Rect>,
    pub actions: Vec<Action>,
    pub children: Vec<NodeId>,
}
```

```rust
pub enum Action {
    Focus,
    Press,
    Increment,
    Decrement,
    SetValue(String),
}
```

---

## MCP Protocol Schema

The MCP interface follows a JSON-RPC style over the selected transport.

### Requests

| Method           | Description                            |
| ---------------- | -------------------------------------- |
| `query_tree`     | Returns the entire accessibility tree. |
| `get_node`       | Returns details for a given node.      |
| `perform_action` | Performs an accessibility action.      |
| `find_by_name`   | Searches the tree for a node by name.  |

### Example Request

```json
{
  "method": "perform_action",
  "params": {
    "node_id": "42",
    "action": "click"
  }
}
```

### Example Response

```json
{
  "result": {
    "success": true
  }
}
```

---

## Security Model

* The MCP server only exposes data for the **current process**.
* No global accessibility access is required.
* No elevated privileges or “assistive device” permissions are needed.
* Intended only for local development and testing.
* The crate should gate the functionality behind a Cargo feature, e.g.:

  ```toml
  [features]
  accessibility_mcp = []
  ```

---

## Implementation Roadmap

### Phase 1: Core + macOS Proof of Concept

* Implement `AccessibilityProvider` for macOS.
* Create a minimal MCP loop with `tokio`.
* Test on a demo app (e.g. Egui).

### Phase 2: Add Windows and Linux Backends

* Use `windows` crate and `atspi` crate.
* Introduce platform auto-detection.

### Phase 3: Normalization via AccessKit Consumer

* Add optional normalization layer for consistent role naming and property mapping.

### Phase 4: Developer Tools Integration

* Add example of coding agent interaction.
* Document how to connect LLM-based agents to the MCP endpoint.

---

## Example Integration

```rust
use accessibility_mcp::start_mcp_server;
use eframe::egui;

fn main() -> eframe::Result {
    let _mcp = start_mcp_server(None)?;
    let options = eframe::NativeOptions::default();
    eframe::run_native("My Egui App", options, Box::new(|_cc| Ok(Box::new(MyApp::default()))))
}
```

---

## License

Dual-licensed under MIT or Apache-2.0, same as AccessKit.

---

## References

* [AccessKit](https://github.com/AccessKit/accesskit)
* [AT-SPI2 D-Bus Specification](https://gitlab.gnome.org/GNOME/at-spi2-core)
* [UI Automation (Microsoft Docs)](https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32)
* [AXAPI (Apple Developer Docs)](https://developer.apple.com/documentation/applicationservices/accessibility)
