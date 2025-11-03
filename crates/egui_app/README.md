# Egui App with Optional Accessibility MCP Server

This is a demonstration application showing how to conditionally include the accessibility MCP server using feature flags.

## Running Without MCP Server

```bash
cargo run -p egui_app
```

The app will display "❌ MCP server is DISABLED" and run as a normal egui application without any additional dependencies.

## Running With MCP Server

```bash
cargo run -p egui_app --features a11y_mcp
```

The app will display "✅ MCP server is ENABLED" and start a Unix socket server that exposes the accessibility tree.

## Testing the MCP Server

When running with the `a11y_mcp` feature:

1. Note the PID displayed in the UI
2. Connect to the socket:
   ```bash
   nc -U /tmp/accessibility_mcp_<PID>.sock
   ```
3. Send MCP requests:
   ```bash
   {"protocol_version":"1.0","method":"query_tree"}
   ```

## Feature Flag Benefits

- **Zero overhead**: When the feature is disabled, the MCP server code is not compiled or included
- **Optional dependency**: `accessibility_mcp` and `tokio` are only pulled in when needed
- **Clean separation**: Production builds can exclude accessibility inspection entirely
- **Development tool**: Enable during testing and debugging, disable for release

## Example Use Cases

### Development
```bash
cargo run -p egui_app --features a11y_mcp
# Test accessibility tree, automate UI testing, verify a11y implementation
```

### Production
```bash
cargo build -p egui_app --release
# Smaller binary, no MCP server overhead
```

### CI/CD Testing
```bash
cargo test -p egui_app --features a11y_mcp
# Run automated accessibility tests
```
