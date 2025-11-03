//! Test client for communicating with an MCP server via stdio
//!
//! This demonstrates how a coding agent would interact with the MCP server.
//!
//! Usage:
//! 1. Start a server: cargo run --example simple_server
//! 2. In another terminal: echo '{"protocol_version":"1.0","method":"query_tree"}' | cargo run --example test_client

use std::io::{self, BufRead, Write};

fn main() {
    eprintln!("MCP Test Client - reading from stdin");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Send a query_tree request
    let request = r#"{"protocol_version":"1.0","method":"query_tree"}"#;
    eprintln!("Sending request: {}", request);

    writeln!(stdout, "{}", request).expect("Failed to write request");
    stdout.flush().expect("Failed to flush");

    // Read response
    eprintln!("Waiting for response...");
    let mut response = String::new();
    stdin
        .lock()
        .read_line(&mut response)
        .expect("Failed to read response");

    eprintln!("Received response:");
    println!("{}", response);

    // Try to pretty-print if it's valid JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
        println!("\nPretty printed:");
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }
}
