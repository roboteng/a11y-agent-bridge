//! Integration tests for MCP server protocol communication

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
