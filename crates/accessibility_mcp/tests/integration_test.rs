//! Integration tests for MCP server protocol communication

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

#[tokio::test]
#[cfg(target_os = "macos")]
async fn test_mcp_protocol_communication() {
    // This test spawns the simple_server example as a subprocess
    // and verifies we can send requests and receive responses

    let mut child = Command::new("cargo")
        .args(&["run", "--example", "simple_server", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn simple_server");

    let mut stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Send a query_tree request
    let request = r#"{"protocol_version":"1.0","method":"query_tree"}"#;
    writeln!(stdin, "{}", request).expect("Failed to write request");
    stdin.flush().expect("Failed to flush");

    // Read the response with a timeout
    let mut response = String::new();
    let read_result = reader.read_line(&mut response);

    // Clean up
    child.kill().ok();

    // Verify we got a response
    assert!(read_result.is_ok(), "Should receive a response");
    assert!(!response.is_empty(), "Response should not be empty");

    // Verify the response is valid JSON
    let json: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    // Verify protocol structure
    assert_eq!(json["protocol_version"], "1.0");
    assert!(
        json["status"].is_string() || json["result"].is_object(),
        "Response should have status or result field"
    );
}

#[test]
fn test_request_serialization() {
    // Test that we can create and serialize valid MCP requests
    use accessibility_mcp::protocol::*;

    let request = Request::QueryTree {
        max_depth: Some(5),
        max_nodes: Some(100),
    };

    let message = Message::request(request);
    let json = serde_json::to_string(&message).expect("Should serialize");

    // Verify the JSON contains expected fields
    assert!(json.contains("query_tree"));
    assert!(json.contains("max_depth"));
    assert!(json.contains("1.0")); // protocol version
}

#[test]
fn test_response_deserialization() {
    // Test that we can parse MCP responses
    let response_json = r#"{
        "protocol_version": "1.0",
        "status": "success",
        "result": {
            "nodes": [{
                "id": "test-123",
                "role": "button",
                "name": "OK",
                "value": null,
                "description": null,
                "bounds": null,
                "actions": [{"type": "press"}],
                "children": []
            }]
        }
    }"#;

    use accessibility_mcp::protocol::Message;
    let parsed: Message = serde_json::from_str(response_json).expect("Should parse valid response");

    assert_eq!(parsed.protocol_version, "1.0");
}
