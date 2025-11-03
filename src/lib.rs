//! Accessibility MCP Server
//!
//! This crate provides a Model Context Protocol (MCP) server that exposes
//! a live view of an application's accessibility tree to coding agents.
//!
//! # Example
//!
//! ```no_run
//! use accessibility_mcp::{start_mcp_server, Config};
//!
//! fn main() -> anyhow::Result<()> {
//!     let _mcp = start_mcp_server(None)?;
//!     // Your app runs here...
//!     Ok(())
//! }
//! ```

mod platform;
mod protocol;
mod server;

pub use protocol::{Action, Node, NodeId, Rect};
pub use server::{start_mcp_server, Config, LogLevel, McpHandle, TransportKind};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_can_be_created() {
        let node = Node {
            id: NodeId::from("test-id"),
            role: "button".to_string(),
            name: Some("Click Me".to_string()),
            value: None,
            description: None,
            bounds: None,
            actions: vec![Action::Press],
            children: vec![],
        };

        assert_eq!(node.id.as_str(), "test-id");
        assert_eq!(node.role, "button");
        assert_eq!(node.actions.len(), 1);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn can_start_mcp_server() {
        // We need a tokio runtime since the server spawns async tasks
        let runtime = tokio::runtime::Runtime::new().unwrap();

        runtime.block_on(async {
            // Starting the server should succeed
            let handle = start_mcp_server(None);
            assert!(handle.is_ok(), "Should be able to start MCP server");

            // Clean shutdown - the handle should drop cleanly
            if let Ok(h) = handle {
                h.shutdown();
            }

            // Give the background task a moment to shut down
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });
    }

    #[test]
    fn can_serialize_request() {
        use protocol::*;

        let request = Request::GetNode {
            node_id: NodeId::from("test-123"),
        };

        let message = Message::request(request);
        let json = serde_json::to_string(&message).expect("Should serialize");

        assert!(json.contains("get_node"));
        assert!(json.contains("test-123"));
        assert!(json.contains("1.0")); // protocol version
    }

    #[test]
    fn can_serialize_response() {
        use protocol::*;

        let node = Node {
            id: NodeId::from("n1"),
            role: "button".to_string(),
            name: Some("OK".to_string()),
            value: None,
            description: None,
            bounds: None,
            actions: vec![Action::Press],
            children: vec![],
        };

        let response = Response::Success {
            result: ResponseData::Node { node: node.clone() },
        };

        let message = Message::response(response);
        let json = serde_json::to_string(&message).expect("Should serialize");

        assert!(json.contains("success"));
        assert!(json.contains("button"));
        assert!(json.contains("OK"));
    }
}
