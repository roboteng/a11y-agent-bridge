# Dioxus App with Optional Accessibility MCP Server

This is a demonstration application showing how to conditionally include the accessibility MCP server using feature flags in a **Dioxus** application.

**Why Dioxus?** Dioxus ships with the Tokio runtime by default, making MCP server integration seamless - no manual runtime creation needed!

## Running Without MCP Server

```bash
cargo run -p dioxus_app
```

The app will display "❌ MCP server is DISABLED" and run as a normal Dioxus application without the MCP dependency.

## Running With MCP Server

```bash
cargo run -p dioxus_app --features a11y_mcp
```

The app will display "✅ MCP server is ENABLED" and start a Unix socket server that exposes the accessibility tree.

## Key Differences from Egui Example

### Egui Approach (Manual Runtime)
```rust
// Egui doesn't use Tokio, so we must create a runtime manually
let runtime = tokio::runtime::Runtime::new()?;
let _guard = runtime.enter();
let _mcp = start_mcp_server(config)?;
```

### Dioxus Approach (Seamless)
```rust
// Dioxus already has Tokio running - just start the server!
let _mcp = start_mcp_server(config)?;
```

**Much cleaner!** Dioxus handles all the async runtime management for you.

## Testing the MCP Server

When running with the `a11y_mcp` feature:

1. Note the PID displayed in the UI (in the details section)
2. Connect to the socket:
   ```bash
   nc -U /tmp/accessibility_mcp_<PID>.sock
   ```
3. Send MCP requests:
   ```bash
   {"protocol_version":"1.0","method":"query_tree"}
   ```

## Accessibility Support

Dioxus uses **AccessKit** for accessibility, just like our MCP server. This means:
- ✅ Full accessibility tree available
- ✅ Native platform accessibility APIs work
- ✅ Screen readers can interact with the app
- ✅ MCP server can inspect and control the UI

## Feature Flag Benefits

- **Zero overhead**: When disabled, MCP server code is not compiled
- **Optional dependency**: `accessibility_mcp` only pulled in when needed
- **Cleaner integration**: No manual Tokio runtime management required
- **Production ready**: Disable for release builds

## Example Use Cases

### Development with MCP
```bash
cargo run -p dioxus_app --features a11y_mcp
# Test accessibility tree, automate UI testing, verify a11y implementation
```

### Production Build
```bash
cargo build -p dioxus_app --release
# Smaller binary, no MCP server overhead
```

### CI/CD Testing
```bash
cargo test -p dioxus_app --features a11y_mcp
# Run automated accessibility tests via MCP protocol
```

## Why This Example Matters

This demonstrates the **ideal pattern** for integrating the MCP server:
1. Framework already uses Tokio → No runtime boilerplate needed
2. Feature flag for optional inclusion
3. Zero impact when disabled
4. Clean, maintainable code

If your GUI framework uses Tokio (Dioxus, Tauri, etc.), this is the recommended integration pattern!
