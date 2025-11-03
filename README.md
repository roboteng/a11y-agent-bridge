# Accessibility MCP Server

A Model Context Protocol (MCP) server that exposes an application's accessibility tree to coding agents and developer tools.

## Overview

This workspace provides:
- **`accessibility_mcp`**: A library crate that provides the MCP server for exposing accessibility trees
- **`egui_app`**: A demo application showcasing optional feature-flag integration

Coding agents can inspect and interact with native application UIs through the same accessibility APIs used by assistive technologies. No modifications to the target application are required - it works with any app that properly implements accessibility.

## Quick Start

### In Your Application

```rust
use accessibility_mcp::start_mcp_server;

fn main() -> anyhow::Result<()> {
    // Start the MCP server
    let _mcp = start_mcp_server(None)?;
    
    // Your app runs here...
    Ok(())
}
```

### Communicating with the Server

The server uses JSON-RPC over stdio by default. Send requests as JSON lines:

**Request:**
```json
{"protocol_version":"1.0","method":"query_tree"}
```

**Response:**
```json
{
  "protocol_version":"1.0",
  "status":"success",
  "result":{
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
```

## Supported Operations

### `query_tree`
Get the accessibility tree (starting from root):
```json
{
  "protocol_version":"1.0",
  "method":"query_tree",
  "max_depth":5,
  "max_nodes":100
}
```

### `get_node`
Get details for a specific node:
```json
{
  "protocol_version":"1.0",
  "method":"get_node",
  "node_id":"0x123456"
}
```

### `perform_action`
Perform an action on a node:
```json
{
  "protocol_version":"1.0",
  "method":"perform_action",
  "node_id":"0x123456",
  "action":{"type":"press"}
}
```

### `find_by_name`
Find nodes by name:
```json
{
  "protocol_version":"1.0",
  "method":"find_by_name",
  "name":"OK"
}
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
# 1. Start the app with MCP server enabled
cargo run -p egui_app --features a11y_mcp

# 2. Query the accessibility tree
echo '{"protocol_version":"1.0","method":"query_tree"}' | nc -U /tmp/accessibility_mcp_<PID>.sock

# 3. Traverse to find a slider (role: "AXSlider")
echo '{"protocol_version":"1.0","method":"get_node","node_id":"<slider_id>"}' | nc -U /tmp/accessibility_mcp_<PID>.sock

# 4. Control the slider
echo '{"protocol_version":"1.0","method":"perform_action","node_id":"<slider_id>","action":{"type":"increment"}}' | nc -U /tmp/accessibility_mcp_<PID>.sock

# 5. Click a button
echo '{"protocol_version":"1.0","method":"perform_action","node_id":"<button_id>","action":{"type":"press"}}' | nc -U /tmp/accessibility_mcp_<PID>.sock
```

This enables automated UI testing, accessibility verification, and remote control of applications!

## Configuration

### Stdio Transport (for headless/CLI tools)

```rust
use accessibility_mcp::start_mcp_server;

let _mcp = start_mcp_server(None)?;  // Uses stdio by default
```

### Unix Socket Transport (for GUI apps)

```rust
use accessibility_mcp::{start_mcp_server, Config, TransportKind};

let config = Config {
    transport: TransportKind::UnixSocket,
    socket_path: None,  // Auto-generates /tmp/accessibility_mcp_<PID>.sock
    ..Default::default()
};

let _mcp = start_mcp_server(Some(config))?;
```

## Examples

### GUI Application with Feature Flag

The egui demo app can run with or without the MCP server:

**Without MCP server** (production mode):
```bash
cargo run -p egui_app
```

**With MCP server** (development/testing mode):
```bash
cargo run -p egui_app --features a11y_mcp
```

When the `a11y_mcp` feature is enabled, the app will display the socket path in the UI. Connect to it:
```bash
# Get the PID from the UI or from ps
echo '{"protocol_version":"1.0","method":"query_tree"}' | nc -U /tmp/accessibility_mcp_<PID>.sock
```

Example response:
```json
{
  "protocol_version": "1.0",
  "status": "success",
  "result": {
    "nodes": [{
      "id": "0x6000008d41b0",
      "role": "AXApplication",
      "name": "egui_app",
      "actions": [{"type": "focus"}],
      "children": ["0x6000008d4200", "0x6000008d4300"]
    }]
  }
}
```

You can then query child nodes, find buttons, sliders, etc., and perform actions on them:
```bash
# Find a slider and increment it
echo '{"protocol_version":"1.0","method":"perform_action","node_id":"0x123abc","action":{"type":"increment"}}' | nc -U /tmp/accessibility_mcp_<PID>.sock
```

### Library Examples

The `accessibility_mcp` library crate includes additional examples:

```bash
# Minimal server (stdio)
cargo run -p accessibility_mcp --example minimal_server

# Test provider directly
cargo run -p accessibility_mcp --example test_provider

# Simple server
cargo run -p accessibility_mcp --example simple_server
```

## Current Limitations

- Bounds/coordinates not extracted yet
- `set_value` action not yet implemented
- `scroll` and `context_menu` actions not yet implemented
- `find_by_name` not implemented (requires manual tree traversal)
- macOS only (Windows and Linux planned)
- Requires accessibility permissions on macOS

## What's Working

- ✅ Full accessibility tree traversal with children enumeration
- ✅ Actions: `focus`, `press`, `increment`, `decrement`
- ✅ Unix socket transport for GUI applications
- ✅ Stdio transport for CLI tools
- ✅ Finding UI elements by traversing the tree
- ✅ Performing actions on buttons, sliders, and other controls
- ✅ Successfully tested with egui applications

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## License

Dual-licensed under MIT or Apache-2.0
