# Accessibility MCP Server

A Model Context Protocol (MCP) server that exposes an application's accessibility tree to coding agents and developer tools.

## Overview

This workspace provides:
- **`accessibility_mcp`**: A library crate that provides the MCP server for exposing accessibility trees
- **`egui_app`**: Demo application with manual Tokio runtime (egui doesn't use async)
- **`dioxus_app`**: Demo application with seamless integration (Dioxus ships with Tokio)

Coding agents can inspect and interact with native application UIs through the same accessibility APIs used by assistive technologies. No modifications to the target application are required - it works with any app that properly implements accessibility.

## Quick Start

### In Your Application

**For applications without Tokio runtime (e.g., egui):**

```rust
use accessibility_mcp::start_all;

fn main() -> anyhow::Result<()> {
    // Starts Tokio runtime and MCP server on http://127.0.0.1:{PORT}
    // Uses OS-assigned arbitrary port for maximum compatibility
    let (_runtime, mcp_handle) = start_all()?;
    
    println!("MCP server listening on port {}", mcp_handle.port);
    
    // Your app runs here...
    Ok(())
}
```

**For applications that already have Tokio runtime (e.g., Dioxus):**

```rust
use accessibility_mcp::start_mcp_server;

fn main() -> anyhow::Result<()> {
    // Start the MCP server on http://127.0.0.1:{PORT}
    // Pass desired port (e.g., 3000) or 0 for OS-assigned port
    let mcp_handle = start_mcp_server(3000)?;
    
    println!("MCP server listening on port {}", mcp_handle.port);
    
    // Your app runs here...
    Ok(())
}
```

### Communicating with the Server

The server uses JSON-RPC over HTTP. Send POST requests to the `/mcp` endpoint:

**Request:**
```bash
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"query_tree":{}}}}'
```

**Response:**
```json
{
  "protocol_version":"1.0",
  "content":{
    "response":{
      "success":{
        "result":{
          "tree":{
            "nodes":[{
              "id":"0x123456",
              "role":"AXApplication",
              "name":"My App",
              "value":null,
              "description":null,
              "bounds":null,
              "actions":["focus"],
              "children":["0x123457","0x123458"]
            }]
          }
        }
      }
    }
  }
}
```

## Supported Operations

### `query_tree`
Get the accessibility tree (starting from root):
```bash
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"query_tree":{"max_depth":5,"max_nodes":100}}}}'
```

### `get_node`
Get details for a specific node:
```bash
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"get_node":{"node_id":"0x123456"}}}}'
```

### `perform_action`
Perform an action on a node:
```bash
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"perform_action":{"node_id":"0x123456","action":{"type":"press"}}}}}'
```

### `find_by_name`
Find nodes by name:
```bash
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"find_by_name":{"name":"OK"}}}}'
```

## Supported Actions

- `focus` - Set keyboard focus
- `press` - Activate/click the element  
- `increment` - Increase value (sliders, steppers)
- `decrement` - Decrease value
- `set_value` - Set text value
- `scroll` - Scroll by given amount
- `context_menu` - Open context menu
- `custom` - Platform-specific action

## Platform Support

| Platform | API | Status |
|----------|-----|--------|
| macOS | AXAPI | ✅ Implemented |
| Windows | UI Automation | 🚧 Planned |
| Linux | AT-SPI2 | 🚧 Planned |

### macOS Permissions

On macOS, the application needs accessibility permissions to query its own accessibility tree:

1. Run your application
2. When prompted, grant accessibility permissions
3. Alternatively, manually add your app in:
   **System Preferences > Privacy & Security > Accessibility**

**Note:** Command-line tools may not be able to access the accessibility API even for self-inspection. GUI applications (like the egui example) work best.

## For Coding Agents

This library is designed to be consumed by AI coding agents like Claude Code. The MCP protocol provides a standardized way to:

1. Inspect UI structure and content
2. Verify accessibility implementation
3. Automate UI testing
4. Provide feedback on accessibility issues

### Example Agent Workflow

1. Agent starts the target application with MCP server enabled
2. Agent sends `query_tree` to understand the UI
3. Agent traverses the tree to locate specific elements (buttons, sliders, etc.)
4. Agent sends `perform_action` to interact with elements
5. Agent verifies expected behavior

### Real Example

```bash
# 1. Start the app with MCP server enabled (using Dioxus for clean integration)
cargo run -p dioxus_app --features a11y_mcp

# Note the port number from the output, e.g., "listening on http://127.0.0.1:3000"

# 2. Query the accessibility tree
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"query_tree":{}}}}'

# 3. Traverse to find a slider (role: "AXSlider")
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"get_node":{"node_id":"<slider_id>"}}}}'

# 4. Control the slider
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"perform_action":{"node_id":"<slider_id>","action":{"type":"increment"}}}}}'

# 5. Click a button
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"perform_action":{"node_id":"<button_id>","action":{"type":"press"}}}}}'
```

This enables automated UI testing, accessibility verification, and remote control of applications!

## Server Address

The server listens on a local HTTP port at:

```
http://127.0.0.1:{PORT}
```

The port can be specified when calling `start_mcp_server(port)`:
- **Port 0**: OS assigns an arbitrary available port (used by `start_all()`)
- **Specific port** (e.g., 3000): Tries that port, or successive ports if unavailable

The URL is automatically logged to stderr when the server starts:

```
[MCP] listening on http://127.0.0.1:3000
```

The actual bound port is available via the `McpHandle.port` field.

## Examples

### GUI Applications with Feature Flag

We provide two demo applications showcasing different integration patterns:

#### Dioxus App (Uses `start_mcp_server`)

Dioxus ships with Tokio, so it uses `start_mcp_server()` directly:

**Without MCP server** (production mode):
```bash
cargo run -p dioxus_app
```

**With MCP server** (development/testing mode):
```bash
cargo run -p dioxus_app --features a11y_mcp
```

The code simply calls:
```rust
let mcp_handle = accessibility_mcp::start_mcp_server(3000)?;
```

#### Egui App (Uses `start_all`)

Egui doesn't have Tokio, so it uses `start_all()` which creates the runtime:

**Without MCP server**:
```bash
cargo run -p egui_app
```

**With MCP server**:
```bash
cargo run -p egui_app --features a11y_mcp
```

The code simply calls:
```rust
let (_runtime, mcp_handle) = accessibility_mcp::start_all()?;
```

When the `a11y_mcp` feature is enabled, the app will display the HTTP URL in the UI. Connect to it:
```bash
# Get the port from the UI or from stderr output
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"query_tree":{}}}}'
```

Example response:
```json
{
  "protocol_version": "1.0",
  "content": {
    "response": {
      "success": {
        "result": {
          "tree": {
            "nodes": [{
              "id": "0x6000008d41b0",
              "role": "AXApplication",
              "name": "egui_app",
              "actions": [{"type": "focus"}],
              "children": ["0x6000008d4200", "0x6000008d4300"]
            }]
          }
        }
      }
    }
  }
}
```

You can then query child nodes, find buttons, sliders, etc., and perform actions on them:
```bash
# Find a slider and increment it
curl -X POST http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"protocol_version":"1.0","content":{"request":{"perform_action":{"node_id":"0x123abc","action":{"type":"increment"}}}}}'
```

### Library Examples

The `accessibility_mcp` library crate includes additional examples:

```bash
# Simple server
cargo run -p accessibility_mcp --example simple_server

# Test provider directly
cargo run -p accessibility_mcp --example test_provider
```

## Current Limitations

- Bounds/coordinates not extracted yet
- `set_value` action not yet implemented
- `scroll` and `context_menu` actions not yet implemented
- macOS only (Windows and Linux planned)
- Requires accessibility permissions on macOS

## What's Working

- ✅ Full accessibility tree traversal with children enumeration
- ✅ Actions: `focus`, `press`, `increment`, `decrement`
- ✅ HTTP transport with JSON-RPC protocol
- ✅ CORS-enabled for web-based clients
- ✅ Finding UI elements by name and traversing the tree
- ✅ Performing actions on buttons, sliders, and other controls
- ✅ Successfully tested with egui and Dioxus applications

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## License

Dual-licensed under MIT or Apache-2.0
