//! Minimal MCP server for testing - no GUI dependencies
//!
//! This creates a simple runtime and starts the MCP server.

fn main() -> anyhow::Result<()> {
    // Create a tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        eprintln!("Starting minimal MCP server on stdio...");

        // Start the MCP server
        let _handle = accessibility_mcp::start_mcp_server(None)?;

        eprintln!("Server ready. Send MCP requests via stdin.");

        // Keep running indefinitely
        let () = std::future::pending().await;

        Ok::<(), anyhow::Error>(())
    })
}
