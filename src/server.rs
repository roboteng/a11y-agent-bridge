//! MCP server implementation

use crate::platform::{create_provider, AccessibilityProvider};
use crate::protocol::{ErrorCode, Message, MessageContent, Request, Response, ResponseData};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

/// Transport mechanism for the MCP server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// Standard input/output (default)
    Stdio,
    /// Unix domain socket
    UnixSocket,
    /// TCP socket
    Tcp,
}

impl Default for TransportKind {
    fn default() -> Self {
        Self::Stdio
    }
}

/// Logging level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        }
    }
}

/// Configuration for the MCP server
#[derive(Debug, Clone)]
pub struct Config {
    pub transport: TransportKind,
    pub port: Option<u16>,
    pub normalize: bool,
    pub log_level: LogLevel,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transport: TransportKind::default(),
            port: None,
            normalize: false,
            log_level: LogLevel::default(),
        }
    }
}

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

/// Start the MCP server
pub fn start_mcp_server(config: Option<Config>) -> Result<McpHandle> {
    let config = config.unwrap_or_default();

    // Initialize logging (ignore if already initialized)
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::from(config.log_level))
        .with_writer(std::io::stderr)
        .try_init();

    tracing::info!("Starting accessibility MCP server");

    // Create the accessibility provider
    let provider = create_provider().context("Failed to create accessibility provider")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Spawn the server task
    match config.transport {
        TransportKind::Stdio => {
            tokio::spawn(run_stdio_server(Arc::new(provider), shutdown_rx));
            eprintln!("[MCP] listening on stdio");
        }
        TransportKind::UnixSocket => {
            anyhow::bail!("Unix socket transport not yet implemented");
        }
        TransportKind::Tcp => {
            anyhow::bail!("TCP transport not yet implemented");
        }
    }

    Ok(McpHandle {
        shutdown_tx: Some(shutdown_tx),
    })
}

/// Run the stdio-based MCP server
async fn run_stdio_server(
    provider: Arc<Box<dyn AccessibilityProvider>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();

        tokio::select! {
            _ = &mut shutdown_rx => {
                tracing::info!("Server shutting down");
                break;
            }
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        // EOF
                        tracing::info!("Stdin closed, shutting down");
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
                            if let Err(e) = stdout.write_all(json.as_bytes()).await {
                                tracing::error!("Failed to write response: {}", e);
                                break;
                            }
                            if let Err(e) = stdout.write_all(b"\n").await {
                                tracing::error!("Failed to write newline: {}", e);
                                break;
                            }
                            if let Err(e) = stdout.flush().await {
                                tracing::error!("Failed to flush stdout: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading from stdin: {}", e);
                        break;
                    }
                }
            }
        }
    }
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
    _provider: &Arc<Box<dyn AccessibilityProvider>>,
    _name: &str,
) -> Response {
    // For now, just return empty list
    // TODO: implement tree traversal and name matching
    Response::Success {
        result: ResponseData::Nodes { nodes: vec![] },
    }
}
