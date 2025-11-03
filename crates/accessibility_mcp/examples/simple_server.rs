//! Simple MCP server without GUI
//!
//! This runs the MCP server and keeps the process alive so you can interact with it.
//!
//! Run with: cargo run --example simple_server
//!
//! Then in another terminal, send requests:
//!   echo '{"protocol_version":"1.0","method":"query_tree"}' | cargo run --example simple_server

use accessibility_mcp::start_mcp_server;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    eprintln!("Starting simple MCP server...");
    eprintln!("Send JSON requests via stdin, responses will be on stdout");
    eprintln!("Press Ctrl+C to exit");
    eprintln!();

    // Start the MCP server
    let _handle = start_mcp_server(None)?;

    // Keep the process alive
    // The server will handle stdin/stdout in the background
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
