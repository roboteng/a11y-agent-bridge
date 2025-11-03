# Accessibility MCP Server

A Model Context Protocol (MCP) server that exposes an application's accessibility tree to coding agents and developer tools.

## Overview

This crate allows coding agents to inspect and interact with native application UIs through the same accessibility APIs used by assistive technologies. No modifications to the target application are required - it works with any app that properly implements accessibility.

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

## For Coding Agents

This library is designed to be consumed by AI coding agents like Claude Code. The MCP protocol provides a standardized way to:

1. Inspect UI structure and content
2. Verify accessibility implementation
3. Automate UI testing
4. Provide feedback on accessibility issues

### Example Agent Workflow

1. Agent starts the target application with MCP server enabled
2. Agent sends `query_tree` to understand the UI
3. Agent sends `find_by_name` to locate specific elements
4. Agent sends `perform_action` to interact with elements
5. Agent verifies expected behavior

## Configuration

```rust
use accessibility_mcp::{start_mcp_server, Config, TransportKind, LogLevel};

let config = Config {
    transport: TransportKind::Stdio,  // or UnixSocket, Tcp
    port: None,
    normalize: false,  // normalize to AccessKit model
    log_level: LogLevel::Info,
};

let _mcp = start_mcp_server(Some(config))?;
```

## Examples

Run the minimal server:
```bash
cargo run --example minimal_server
```

Then send requests:
```bash
echo '{"protocol_version":"1.0","method":"query_tree"}' | cargo run --example minimal_server
```

## Current Limitations

- Children enumeration not fully implemented
- Bounds/coordinates not extracted yet
- Limited action support (Focus, Press only)
- `find_by_name` not implemented
- macOS only

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## License

Dual-licensed under MIT or Apache-2.0
