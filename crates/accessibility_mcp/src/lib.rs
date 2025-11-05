//! Accessibility MCP Server
//!
//! This crate provides a Model Context Protocol (MCP) server that exposes
//! a live view of an application's accessibility tree to coding agents.
//!
//! # Example
//!
//! ```no_run
//! use accessibility_mcp::start_mcp_server;
//!
//! fn main() -> anyhow::Result<()> {
//!     // Starts server on /tmp/accessibility_mcp_{PID}.sock
//!     let _mcp = start_mcp_server()?;
//!     // Your app runs here...
//!     Ok(())
//! }
//! ```

pub mod platform;
pub mod protocol;
mod server;

pub use protocol::{Action, Node, NodeId, Rect};
pub use server::{start_mcp_server, McpHandle};

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

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn can_start_mcp_server() {
        let handle = start_mcp_server().expect("Should be able to start MCP server");
        handle.shutdown();
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
