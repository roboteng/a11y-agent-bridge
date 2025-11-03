//! Platform-specific accessibility backends

use crate::protocol::{Action, Node, NodeId};
use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOSProvider;

/// Trait for consuming accessibility data from platform APIs
pub trait AccessibilityProvider: Send + Sync {
    /// Get the root accessibility node for this process
    fn get_root(&self) -> Result<Node>;

    /// Get all children of a given node
    fn get_children(&self, node_id: &NodeId) -> Result<Vec<Node>>;

    /// Get a specific node by ID
    fn get_node(&self, node_id: &NodeId) -> Result<Node>;

    /// Perform an accessibility action on a node
    fn perform_action(&self, node_id: &NodeId, action: &Action) -> Result<()>;
}

/// Create the appropriate provider for the current platform
pub fn create_provider() -> Result<Box<dyn AccessibilityProvider>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSProvider::new()?))
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("Unsupported platform")
    }
}
