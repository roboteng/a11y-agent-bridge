//! MCP protocol data structures and request/response types

use serde::{Deserialize, Serialize};

/// A unique identifier for an accessibility node.
///
/// The format is platform-specific but guaranteed to be stable
/// for the lifetime of the node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Rectangle representing the bounds of a node in screen coordinates.
/// Origin is top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Actions that can be performed on accessibility nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Set focus to this element
    Focus,
    /// Press/activate this element (click, invoke)
    Press,
    /// Increment a numeric value
    Increment,
    /// Decrement a numeric value
    Decrement,
    /// Set a text value
    SetValue { value: String },
    /// Scroll by given amounts
    Scroll { x: f64, y: f64 },
    /// Open context menu
    ContextMenu,
    /// Platform-specific custom action
    Custom { name: String },
}

/// An accessibility tree node with normalized properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub bounds: Option<Rect>,
    pub actions: Vec<Action>,
    pub children: Vec<NodeId>,
}

/// MCP request types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Request {
    /// Query the accessibility tree
    QueryTree {
        #[serde(default)]
        max_depth: Option<usize>,
        #[serde(default)]
        max_nodes: Option<usize>,
    },
    /// Get a specific node by ID
    GetNode { node_id: NodeId },
    /// Perform an action on a node
    PerformAction { node_id: NodeId, action: Action },
    /// Find nodes by name (substring match)
    FindByName { name: String },
}

/// MCP response types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Success { result: ResponseData },
    Error { error: ErrorInfo },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseData {
    Tree { nodes: Vec<Node> },
    Node { node: Node },
    ActionResult { success: bool },
    Nodes { nodes: Vec<Node> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    Transient,
    InvalidAction,
    Internal,
}

/// MCP protocol envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub protocol_version: String,
    #[serde(flatten)]
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Request(Request),
    Response(Response),
}

impl Message {
    pub const PROTOCOL_VERSION: &'static str = "1.0";

    pub fn request(req: Request) -> Self {
        Self {
            protocol_version: Self::PROTOCOL_VERSION.to_string(),
            content: MessageContent::Request(req),
        }
    }

    pub fn response(resp: Response) -> Self {
        Self {
            protocol_version: Self::PROTOCOL_VERSION.to_string(),
            content: MessageContent::Response(resp),
        }
    }

    pub fn success(data: ResponseData) -> Self {
        Self::response(Response::Success { result: data })
    }

    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::response(Response::Error {
            error: ErrorInfo {
                code,
                message: message.into(),
            },
        })
    }
}
