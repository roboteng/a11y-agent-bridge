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
|  (JSON-RPC over HTTP)     |
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
/// MCP requests over a Unix domain socket at /tmp/accessibility_mcp_{PID}.sock
///
/// Note: Requires an existing Tokio runtime. For applications without Tokio,
/// use `start_all()` instead which creates the runtime for you.
///
/// Returns a handle that can be used to stop the server.
pub fn start_mcp_server() -> anyhow::Result<McpHandle>;

/// Convenience function that creates a Tokio runtime and starts the MCP server.
///
/// This is the recommended function for applications that don't already have
/// a Tokio runtime (e.g., egui applications). For applications that already
/// use Tokio (e.g., Dioxus), use `start_mcp_server()` directly.
///
/// Returns both the runtime (which must be kept alive) and the server handle.
pub fn start_all() -> anyhow::Result<(tokio::runtime::Runtime, McpHandle)>;
```

The HTTP port is specified as a parameter to `start_mcp_server(port)`:
- Port 0 lets the OS assign an arbitrary port (used by `start_all()`)
- A specific port (e.g., 3000) will be attempted, with fallback to successive ports if unavailable

The actual bound port is available via `McpHandle.port`.

#### `McpHandle`

```rust
pub struct McpHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub port: u16,
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

# Accessibility MCP Server — Design Clarifications and Open Questions

This document expands on `ARCHITECTURE.md` and clarifies several design questions
around process identification, transport, concurrency, error handling, and other
implementation subtleties.

---

## 1. Process Identification & Bootstrapping

### How the MCP Server Identifies the Host App

- **macOS:** Use `AXUIElementCreateApplication(getpid())` to obtain the root accessibility object.
- **Windows:** Use `GetCurrentProcess()` + `GetForegroundWindow()` to identify the main HWND, then
  `IUIAutomation::ElementFromHandle(hwnd)` as the root.
- **Linux:** Use the process’s `pid` and application name to locate its root AT-SPI node
  via the D-Bus registry (`/org/a11y/atspi/accessible/root`).

In all cases, **the current process’s PID is used as the key identifier**, meaning
the server always scopes itself to “the app it’s running inside.”

### Timing and Accessibility Tree Initialization

If the MCP server is started **before the accessibility tree exists**, it will:
1. Attempt to connect to the platform API.
2. If the root element is not yet available, retry a limited number of times (e.g. exponential backoff for up to 2 seconds).
3. Once successful, it holds a reference to the root and can answer queries.

In frameworks using AccessKit, `start_mcp_server()` should be called **after**
the platform adapter has been initialized. For other frameworks, this retry behavior
should suffice.

---

## 2. Transport Layer

### Supported Transports

| Transport | Use Case | Notes |
|------------|-----------|-------|
| **HTTP** | Default and only transport. Suitable for all use cases. | Port is specified via `start_mcp_server(port)` parameter. Use 0 for OS-assigned port. Logged on startup. Accessible via `http://127.0.0.1:{PORT}/mcp` |

### Discovery

By default, the server prints its transport details to stderr in a structured form:

```
[MCP] listening on http://127.0.0.1:3000
```

Agents can discover the port either from stderr output or via the `McpHandle.port` field.
The server uses HTTP with JSON-RPC protocol and includes CORS headers for web-based clients.
Later versions may include an environment-variable-based registration system (`ACCESSIBILITY_MCP_PORT`).

---

## 3. Node ID Model

### Generation and Stability

- Each platform backend assigns a **stable identifier** for each node.
- `NodeId` is a string wrapper (`String`), and platform backends decide the format.

| Platform | NodeId Source | Stability |
|-----------|----------------|-----------|
| macOS | `AXUIElementRef` pointer value (converted to string) | Stable during element lifetime. |
| Windows | `RuntimeId` from UIA (array of integers) | Stable unless UIA recreates subtree. |
| Linux | AT-SPI object path (`/org/a11y/atspi/accessible/...`) | Globally unique while object exists. |

The MCP layer maintains a small **ID cache** so repeated queries return the same NodeId
when possible. If a node disappears, the cache entry expires automatically.

---

## 4. Concurrency and Threading

### MCP Server Threading Model

- The MCP server runs in a **background Tokio task**.
- Each incoming request is processed asynchronously.
- Platform backends use an internal mutex or channel to serialize API calls.

### UI Thread Safety

Most accessibility APIs are **thread-safe** for reading,
but not always for mutations (e.g., actions).
To ensure safety:
- All platform API calls are made from a **dedicated worker thread**.
- The main thread is never blocked by accessibility queries.

A per-request timeout (default: 1 second) ensures long calls don’t hang.

---

## 5. Error Handling

### Error Taxonomy

| Category | Description | Example |
|-----------|--------------|----------|
| `NotFound` | Node no longer exists | Node removed before query completed |
| `PermissionDenied` | Platform denied access | macOS privacy restriction |
| `Transient` | Temporary backend failure | DBus timeout |
| `InvalidAction` | Action unsupported for node | Click on static label |
| `Internal` | Unexpected runtime error | Panic in backend thread |

### Recovery Strategy

- Transient errors are retried once.
- Permanent errors are returned to the agent as structured MCP errors.
- If the backend becomes unavailable (e.g., AT-SPI service restart), the server
  automatically reinitializes its connection.

---

## 6. Dynamic Tree Updates

Tree updates are **not pushed** to the agent initially.

Instead:
- The agent may re-query nodes at any time.
- The `get_node` call checks whether the NodeId is still valid.
- A future version may support a “subscribe to changes” stream using AccessKit’s
  incremental update model.

For now, **polling** is the expected strategy.

---

## 7. Performance and Scalability

- `query_tree` accepts optional parameters:
  - `max_depth`: integer
  - `max_nodes`: integer
- Large trees are traversed lazily, yielding partial results.
- Internal pagination is supported through `continuation_token`s.

A full tree traversal is only recommended for debugging or static inspection.

---

## 8. Platform Permission Requirements

| Platform | Expected Behavior | Notes |
|-----------|------------------|--------|
| macOS | May prompt user if global accessibility access is required. | Self-inspection is usually allowed without prompt, but behavior varies by OS version. |
| Windows | UIA access to self is unrestricted. | Cross-process queries require admin privileges. |
| Linux | AT-SPI access to self always allowed if app registered. | Registration handled by AccessKit or toolkit. |

If permissions fail, the server emits a structured warning but remains running.

---

## 9. Action Semantics and Safety

The `Action` enum is intentionally minimal.
Actions are validated per node, based on the platform’s supported actions list.

Additional actions may include:

```rust
pub enum Action {
    Focus,
    Press,
    Increment,
    Decrement,
    SetValue(String),
    Scroll { x: f64, y: f64 },
    ContextMenu,
    Custom(String),
}
````

Destructive actions (e.g. `SetValue`) are never retried automatically.
The agent is expected to validate intent before issuing them.

---

## 10. Coordinate Systems

* All coordinates are normalized to **screen coordinates** (origin top-left).
* Each `Rect` includes a `unit` field indicating whether it’s in pixels or DIP (device-independent pixels).
* Platform conversions:

  * macOS: Convert from CoreGraphics (bottom-left origin).
  * Windows: Direct pixels via `BoundingRectangle`.
  * Linux: From AT-SPI’s extents in screen coordinates.

---

## 11. Testing and Debugging

### Unit Tests

* Mock backends implementing `AccessibilityProvider`.
* Test tree traversal and action dispatch independently of platform APIs.

### Integration Tests

* Launch sample apps (e.g. Egui demo).
* Verify the server responds with expected node structure.

### Developer Tools

* Include a CLI utility:

  ```bash
  cargo run --example dump_tree
  ```

  This connects to the MCP server and prints a readable tree.

### Debug Logging

Enable via environment variable:

```
RUST_LOG=accessibility_mcp=debug
```

---

## 12. Versioning

The MCP protocol includes a top-level version field:

```json
{ "protocol_version": "1.0" }
```

* Minor version bumps (1.x) are backward compatible.
* Major version bumps (2.x) may change schema.
* The server rejects unknown major versions.

---

## 13. Multi-Window and Multi-Process Support

* The root element returned by `get_root()` may represent either:

  * The top-level “application” element (multiple windows)
  * A single main window, depending on platform conventions.

To handle multi-window apps:

* `get_root()` returns a synthetic node representing the app.
* Child nodes correspond to individual top-level windows.

Multi-process support (e.g., Chromium-like architecture) is deferred to future work.

---

## 14. Lifecycle Management

When `McpHandle` is dropped:

* The server shuts down gracefully.
* All transport connections are closed.
* Platform backends release native handles (COM uninit, DBus disconnect).

This ensures no background threads linger after the app exits.

---

## 15. Tree Normalization Consistency

The `consumer` layer maps platform roles and properties into
AccessKit-like roles (`Role::Button`, `Role::TextField`, etc.) using
lookup tables derived from `accesskit_consumer`.

Ambiguous cases (e.g. macOS “group” vs Windows “pane”) are resolved via a priority rule:

* Prefer semantic match (`Role::Group`).
* Fall back to generic `Role::Unknown`.

---

## 16. Security Model Extensions

Even within the current process, precautions include:

* **Query throttling:** Maximum 100 requests per second.
* **Redaction:** Any node marked with role “password” or “secure text field”
  returns no textual content.
* **Rate limiting:** Gradual backoff on repeated identical queries.

The server will **never** execute arbitrary code or shell commands
on behalf of the client.

---

## 17. Integration Testing Reality Check

* **Egui:** Has partial AccessKit integration; sufficient for prototype.
* **Bevy:** Uses AccessKit through winit, works for core elements.
* **Other Frameworks:** As long as the OS accessibility tree is populated,
  the server will function.

Minimum requirement: the app must register itself with the OS accessibility framework.

---

## 18. Outstanding Questions

* Should the MCP server expose a “subscribe to updates” feature?
* Should NodeId stability be guaranteed across sessions?
* Is it desirable to expose raw platform-specific metadata for debugging?

These will be decided after initial prototype validation.

---

## Summary

The clarifications above tighten up the architectural design into something
that can be implemented confidently. The main remaining risks are:

* Platform-specific permission differences (macOS).
* Handling of extremely large accessibility trees.
* Ensuring stable NodeId mapping in dynamic UIs.

With these considerations addressed, the project can proceed to a Phase 1 prototype
on macOS using HTTP transport and expand from there.
