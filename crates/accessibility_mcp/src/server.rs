//! MCP server implementation

use crate::platform::{create_provider, AccessibilityProvider};
use crate::protocol::{ErrorCode, Message, MessageContent, Request, Response, ResponseData};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

/// Handle for controlling the MCP server
pub struct McpHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl McpHandle {
    /// Shutdown the server gracefully
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for McpHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

pub fn start_all() -> Result<(Runtime, McpHandle)> {
    // Create a Tokio runtime for the MCP server
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _runtime_guard = runtime.enter();

    // Start the MCP server before creating the app
    // Listens on /tmp/accessibility_mcp_{PID}.sock
    let handle = start_mcp_server().expect("Failed to start MCP server");

    // Keep the runtime alive
    Ok((runtime, handle))
}

/// Start the MCP server on a Unix socket
///
/// The server will listen on `/tmp/accessibility_mcp_{PID}.sock`
/// where PID is the process ID of the calling application.
pub fn start_mcp_server() -> Result<McpHandle> {
    // Initialize logging (ignore if already initialized)
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .try_init();

    tracing::info!("Starting accessibility MCP server");

    // Create the accessibility provider
    let provider = create_provider().context("Failed to create accessibility provider")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Generate socket path based on PID
    let pid = std::process::id();
    let socket_path = PathBuf::from(format!("/tmp/accessibility_mcp_{}.sock", pid));

    // Spawn the Unix socket server
    tokio::spawn(run_unix_socket_server(
        Arc::new(provider),
        shutdown_rx,
        socket_path.clone(),
    ));

    tracing::info!("Unix socket server listening on {}", socket_path.display());
    eprintln!("[MCP] listening on unix socket: {}", socket_path.display());

    Ok(McpHandle {
        shutdown_tx: Some(shutdown_tx),
    })
}

/// Handle a single MCP request
async fn handle_request(provider: &Arc<Box<dyn AccessibilityProvider>>, line: &str) -> Message {
    // Parse the request
    let message: Message = match serde_json::from_str(line) {
        Ok(msg) => msg,
        Err(e) => {
            return Message::error(ErrorCode::Internal, format!("Invalid JSON: {}", e));
        }
    };

    // Check protocol version
    if message.protocol_version != Message::PROTOCOL_VERSION {
        return Message::error(
            ErrorCode::Internal,
            format!("Unsupported protocol version: {}", message.protocol_version),
        );
    }

    // Extract request
    let request = match message.content {
        MessageContent::Request(req) => req,
        MessageContent::Response(_) => {
            return Message::error(ErrorCode::Internal, "Expected request, got response");
        }
    };

    // Handle the request
    let response = match request {
        Request::QueryTree {
            max_depth,
            max_nodes,
        } => handle_query_tree(provider, max_depth, max_nodes).await,
        Request::GetNode { node_id } => handle_get_node(provider, &node_id).await,
        Request::PerformAction { node_id, action } => {
            handle_perform_action(provider, &node_id, &action).await
        }
        Request::FindByName { name } => handle_find_by_name(provider, &name).await,
        Request::Initialize {
            protocol_version,
            capabilities,
        } => handle_initialize(protocol_version, capabilities).await,
        Request::ToolsList => handle_tools_list().await,
    };

    Message::response(response)
}

async fn handle_query_tree(
    provider: &Arc<Box<dyn AccessibilityProvider>>,
    _max_depth: Option<usize>,
    _max_nodes: Option<usize>,
) -> Response {
    match provider.get_root() {
        Ok(root) => Response::Success {
            result: ResponseData::Tree { nodes: vec![root] },
        },
        Err(e) => Response::Error {
            error: crate::protocol::ErrorInfo {
                code: ErrorCode::Internal,
                message: format!("Failed to get root: {}", e),
            },
        },
    }
}

async fn handle_get_node(
    provider: &Arc<Box<dyn AccessibilityProvider>>,
    node_id: &crate::protocol::NodeId,
) -> Response {
    match provider.get_node(node_id) {
        Ok(node) => Response::Success {
            result: ResponseData::Node { node },
        },
        Err(e) => Response::Error {
            error: crate::protocol::ErrorInfo {
                code: ErrorCode::NotFound,
                message: format!("Node not found: {}", e),
            },
        },
    }
}

async fn handle_perform_action(
    provider: &Arc<Box<dyn AccessibilityProvider>>,
    node_id: &crate::protocol::NodeId,
    action: &crate::protocol::Action,
) -> Response {
    match provider.perform_action(node_id, action) {
        Ok(()) => Response::Success {
            result: ResponseData::ActionResult { success: true },
        },
        Err(e) => Response::Error {
            error: crate::protocol::ErrorInfo {
                code: ErrorCode::InvalidAction,
                message: format!("Failed to perform action: {}", e),
            },
        },
    }
}

async fn handle_find_by_name(
    provider: &Arc<Box<dyn AccessibilityProvider>>,
    name: &str,
) -> Response {
    // Get the root node and traverse the tree
    let root = match provider.get_root() {
        Ok(r) => r,
        Err(e) => {
            return Response::Error {
                error: crate::protocol::ErrorInfo {
                    code: ErrorCode::Internal,
                    message: format!("Failed to get root: {}", e),
                },
            }
        }
    };

    // Perform breadth-first search to find matching nodes
    let mut matches = Vec::new();
    let mut to_visit = vec![root];
    let mut visited = std::collections::HashSet::new();

    // Limit search to prevent infinite loops
    const MAX_NODES: usize = 1000;
    let mut nodes_checked = 0;

    while let Some(node) = to_visit.pop() {
        if nodes_checked >= MAX_NODES {
            tracing::warn!("find_by_name: hit max nodes limit of {}", MAX_NODES);
            break;
        }
        nodes_checked += 1;

        // Skip if already visited (prevent cycles)
        if !visited.insert(node.id.clone()) {
            continue;
        }

        // Check if this node matches (case-insensitive substring match)
        if let Some(node_name) = &node.name {
            if node_name.to_lowercase().contains(&name.to_lowercase()) {
                matches.push(node.clone());
            }
        }

        // Add children to the queue
        for child_id in &node.children {
            match provider.get_node(child_id) {
                Ok(child) => to_visit.push(child),
                Err(e) => {
                    tracing::debug!("Failed to get child node {:?}: {}", child_id, e);
                    // Continue with other children
                }
            }
        }
    }

    Response::Success {
        result: ResponseData::Nodes { nodes: matches },
    }
}

async fn handle_initialize(
    protocol_version: Option<String>,
    _capabilities: Option<serde_json::Value>,
) -> Response {
    // Validate protocol version if provided
    if let Some(version) = protocol_version {
        if !version.starts_with("1.") {
            return Response::Error {
                error: crate::protocol::ErrorInfo {
                    code: ErrorCode::Internal,
                    message: format!("Unsupported protocol version: {}", version),
                },
            };
        }
    }

    Response::Success {
        result: ResponseData::Initialize {
            protocol_version: Message::PROTOCOL_VERSION.to_string(),
            capabilities: crate::protocol::Capabilities {
                tools: Some(crate::protocol::ToolsCapability {
                    list_changed: false,
                }),
            },
            server_info: crate::protocol::ServerInfo {
                name: "accessibility_mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        },
    }
}

async fn handle_tools_list() -> Response {
    use crate::protocol::Tool;

    let tools = vec![
        Tool {
            name: "query_tree".to_string(),
            description: "Query the accessibility tree starting from the root node".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum depth to traverse (optional)"
                    },
                    "max_nodes": {
                        "type": "integer",
                        "description": "Maximum number of nodes to return (optional)"
                    }
                }
            }),
        },
        Tool {
            name: "get_node".to_string(),
            description: "Get details for a specific accessibility node by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique identifier of the node"
                    }
                },
                "required": ["node_id"]
            }),
        },
        Tool {
            name: "perform_action".to_string(),
            description: "Perform an accessibility action on a node".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "node_id": {
                        "type": "string",
                        "description": "The unique identifier of the node"
                    },
                    "action": {
                        "type": "object",
                        "description": "The action to perform",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["focus", "press", "increment", "decrement", "set_value", "scroll", "context_menu", "custom"]
                            }
                        },
                        "required": ["type"]
                    }
                },
                "required": ["node_id", "action"]
            }),
        },
        Tool {
            name: "find_by_name".to_string(),
            description: "Find accessibility nodes by name (substring match)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name or partial name to search for"
                    }
                },
                "required": ["name"]
            }),
        },
    ];

    Response::Success {
        result: ResponseData::Tools { tools },
    }
}

/// Run the Unix socket-based MCP server
#[cfg(unix)]
async fn run_unix_socket_server(
    provider: Arc<Box<dyn AccessibilityProvider>>,
    mut shutdown_rx: oneshot::Receiver<()>,
    socket_path: PathBuf,
) {
    use tokio::net::UnixListener;

    // Remove old socket if it exists
    let _ = std::fs::remove_file(&socket_path);

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind Unix socket: {}", e);
            return;
        }
    };

    tracing::info!("Unix socket server listening on {}", socket_path.display());

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("Unix socket server shutting down");
                let _ = std::fs::remove_file(&socket_path);
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, _addr)) => {
                        let provider = Arc::clone(&provider);
                        tokio::spawn(handle_unix_socket_connection(provider, stream));
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept connection: {}", e);
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
async fn handle_unix_socket_connection(
    provider: Arc<Box<dyn AccessibilityProvider>>,
    stream: tokio::net::UnixStream,
) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();

        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF - client disconnected
                break;
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Process the request
                let response = handle_request(&provider, trimmed).await;

                // Send response
                if let Ok(json) = serde_json::to_string(&response) {
                    if let Err(e) = writer.write_all(json.as_bytes()).await {
                        tracing::error!("Failed to write response: {}", e);
                        break;
                    }
                    if let Err(e) = writer.write_all(b"\n").await {
                        tracing::error!("Failed to write newline: {}", e);
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        tracing::error!("Failed to flush: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Error reading from socket: {}", e);
                break;
            }
        }
    }
}
