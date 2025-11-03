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

mod protocol;
mod server;
mod platform;

pub use protocol::{Node, NodeId, Action, Rect};
pub use server::{start_mcp_server, Config, McpHandle, TransportKind, LogLevel};

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
}
