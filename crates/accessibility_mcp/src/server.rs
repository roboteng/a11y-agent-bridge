//! MCP server implementation

use crate::platform::{create_provider, AccessibilityProvider};
use crate::protocol::{ErrorCode, Message, MessageContent, Request, Response, ResponseData};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response as AxumResponse},
    routing::post,
    Json, Router,
};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

/// Handle for controlling the MCP server
pub struct McpHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// The port the HTTP server is listening on
    pub port: u16,
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
    // Initialize logging (ignore if already initialized)
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .try_init();

    // Create a Tokio runtime for the MCP server
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _runtime_guard = runtime.enter();

    // Start the MCP server before creating the app
    // Listens on http://127.0.0.1:{PORT}
    // Use port 0 to let the OS assign an arbitrary available port
    let handle = start_mcp_server(0).expect("Failed to start MCP server");

    // Keep the runtime alive
    Ok((runtime, handle))
}

/// Start the MCP server on a local HTTP port
///
/// The server will listen on `http://127.0.0.1:{PORT}`
///
/// # Arguments
///
/// * `port` - The port to bind to. If 0, the OS will assign an arbitrary available port.
///            If the specified port is unavailable, will try successive ports up to port+100.
pub fn start_mcp_server(port: u16) -> Result<McpHandle> {
    tracing::info!("Starting accessibility MCP server");

    // Create the accessibility provider
    let provider = create_provider().context("Failed to create accessibility provider")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Determine the actual port to use
    let actual_port = if port == 0 {
        // Let the OS assign an arbitrary port
        0
    } else {
        // Try to find an available port starting from the requested port
        find_available_port(port, port + 100)
    };

    // Spawn the HTTP server
    let (port_tx, port_rx) = oneshot::channel();
    tokio::spawn(run_http_server(
        Arc::new(provider),
        shutdown_rx,
        actual_port,
        port_tx,
    ));

    // Wait for the server to bind and get the actual port
    let bound_port = port_rx
        .blocking_recv()
        .context("Failed to get bound port")?;

    tracing::info!("HTTP server listening on http://127.0.0.1:{}", bound_port);
    eprintln!("[MCP] listening on http://127.0.0.1:{}", bound_port);

    Ok(McpHandle {
        shutdown_tx: Some(shutdown_tx),
        port: bound_port,
    })
}

/// Find an available port in the given range
fn find_available_port(start: u16, end: u16) -> u16 {
    for port in start..=end {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    // Fallback to start port if none available
    start
}

/// Handle a single MCP request
async fn handle_request(
    provider: &Arc<Box<dyn AccessibilityProvider>>,
    message: Message,
) -> Message {
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

/// Shared state for the HTTP server
#[derive(Clone)]
struct AppState {
    provider: Arc<Box<dyn AccessibilityProvider>>,
}

/// HTTP handler for MCP requests
async fn mcp_handler(
    State(state): State<AppState>,
    Json(message): Json<Message>,
) -> Result<Json<Message>, AppError> {
    let response = handle_request(&state.provider, message).await;
    Ok(Json(response))
}

/// Error wrapper for HTTP responses
struct AppError(String);

impl IntoResponse for AppError {
    fn into_response(self) -> AxumResponse {
        (
            StatusCode::BAD_REQUEST,
            Json(Message::error(ErrorCode::Internal, self.0)),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: std::error::Error,
{
    fn from(err: E) -> Self {
        AppError(err.to_string())
    }
}

/// Run the HTTP-based MCP server
async fn run_http_server(
    provider: Arc<Box<dyn AccessibilityProvider>>,
    shutdown_rx: oneshot::Receiver<()>,
    port: u16,
    port_tx: oneshot::Sender<u16>,
) {
    let state = AppState { provider };

    let app = Router::new()
        .route("/mcp", post(mcp_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to {}: {}", addr, e);
            return;
        }
    };

    // Get the actual bound port (important when port 0 is used)
    let bound_port = listener.local_addr().unwrap().port();
    tracing::info!("HTTP server listening on http://127.0.0.1:{}", bound_port);

    // Send the bound port back to the caller
    let _ = port_tx.send(bound_port);

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
        tracing::info!("HTTP server shutting down");
    });

    if let Err(e) = server.await {
        tracing::error!("Server error: {}", e);
    }
}
