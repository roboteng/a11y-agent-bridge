//! macOS accessibility backend using AXAPI

use crate::protocol::{Action, Node, NodeId};
use anyhow::{Context, Result};
use core_foundation::base::{CFType, TCFType};
use core_foundation::string::{CFString, CFStringRef};

use std::collections::HashMap;
use std::sync::Mutex;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
}

type AXUIElementRef = *const std::ffi::c_void;
type AXError = i32;
type CFTypeRef = *const std::ffi::c_void;

const K_AX_ERROR_SUCCESS: AXError = 0;
const K_AX_ERROR_API_DISABLED: AXError = -25208;
const K_AX_ERROR_NO_VALUE: AXError = -25209;

// Common AX attribute constants
const K_AX_ROLE_ATTRIBUTE: &str = "AXRole";
const K_AX_TITLE_ATTRIBUTE: &str = "AXTitle";
const K_AX_VALUE_ATTRIBUTE: &str = "AXValue";
const K_AX_DESCRIPTION_ATTRIBUTE: &str = "AXDescription";
const K_AX_CHILDREN_ATTRIBUTE: &str = "AXChildren";

pub struct MacOSProvider {
    root: AXUIElementRef,
    /// Cache mapping NodeId strings to AXUIElementRef pointers
    element_cache: Mutex<HashMap<String, AXUIElementRef>>,
}

impl MacOSProvider {
    pub fn new() -> Result<Self> {
        // Try to get the root element with retry logic
        let root = unsafe { AXUIElementCreateApplication(std::process::id() as i32) };

        if root.is_null() {
            anyhow::bail!("Failed to create AX application element");
        }

        Ok(Self {
            root,
            element_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Convert AXUIElementRef pointer to NodeId
    fn element_to_node_id(&self, element: AXUIElementRef) -> NodeId {
        let id = format!("{:p}", element);
        NodeId::from(id)
    }

    /// Look up AXUIElementRef from NodeId
    fn node_id_to_element(&self, node_id: &NodeId) -> Result<AXUIElementRef> {
        let cache = self.element_cache.lock().unwrap();
        cache
            .get(node_id.as_str())
            .copied()
            .context("Node ID not found in cache")
    }

    /// Cache an element with its NodeId
    fn cache_element(&self, element: AXUIElementRef) -> NodeId {
        let node_id = self.element_to_node_id(element);
        let mut cache = self.element_cache.lock().unwrap();
        cache.insert(node_id.as_str().to_string(), element);
        node_id
    }

    /// Get a string attribute from an AX element
    unsafe fn get_string_attribute(&self, element: AXUIElementRef, attr: &str) -> Option<String> {
        let attr_name = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null();

        let result =
            AXUIElementCopyAttributeValue(element, attr_name.as_concrete_TypeRef(), &mut value);

        if result == K_AX_ERROR_API_DISABLED {
            // Only log this once to avoid spam
            static WARNED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                tracing::warn!(
                    "Accessibility API is disabled. This app may need to be granted accessibility \
                     permissions in System Preferences > Privacy & Security > Accessibility."
                );
            }
            return None;
        }

        if result == K_AX_ERROR_NO_VALUE {
            // Attribute doesn't exist on this element, which is normal
            return None;
        }

        if result == K_AX_ERROR_SUCCESS && !value.is_null() {
            let cf_value = CFType::wrap_under_create_rule(value);

            // Try to downcast to CFString
            if let Some(string) = cf_value.downcast::<CFString>() {
                return Some(string.to_string());
            } else {
                // Debug: what type did we get?
                tracing::debug!("Attribute {} returned non-string type", attr);
            }
        } else if result != K_AX_ERROR_SUCCESS {
            tracing::debug!("Failed to get attribute {}: error {}", attr, result);
        }

        None
    }

    /// Get children elements from an AX element
    unsafe fn get_children_elements(&self, element: AXUIElementRef) -> Vec<AXUIElementRef> {
        use core_foundation::array::{CFArray, CFArrayRef};
        use core_foundation::base::TCFType;

        let attr_name = CFString::new(K_AX_CHILDREN_ATTRIBUTE);
        let mut value: CFTypeRef = std::ptr::null();

        let result =
            AXUIElementCopyAttributeValue(element, attr_name.as_concrete_TypeRef(), &mut value);

        if result == K_AX_ERROR_NO_VALUE {
            // No children, which is normal
            return Vec::new();
        }

        if result != K_AX_ERROR_SUCCESS || value.is_null() {
            tracing::debug!("Failed to get children: error {}", result);
            return Vec::new();
        }

        // Cast to CFArray
        let array_ref = value as CFArrayRef;
        let array = CFArray::<CFType>::wrap_under_get_rule(array_ref);

        let mut children = Vec::new();
        for i in 0..array.len() {
            if let Some(item) = array.get(i) {
                // The item should be an AXUIElementRef
                let child_element = item.as_CFTypeRef() as AXUIElementRef;
                children.push(child_element);
            }
        }

        children
    }

    /// Convert AXUIElementRef to Node
    fn element_to_node(&self, element: AXUIElementRef) -> Result<Node> {
        let node_id = self.cache_element(element);

        unsafe {
            let role = self
                .get_string_attribute(element, K_AX_ROLE_ATTRIBUTE)
                .unwrap_or_else(|| "unknown".to_string());

            let name = self.get_string_attribute(element, K_AX_TITLE_ATTRIBUTE);
            let value = self.get_string_attribute(element, K_AX_VALUE_ATTRIBUTE);
            let description = self.get_string_attribute(element, K_AX_DESCRIPTION_ATTRIBUTE);

            // Get children
            let child_elements = self.get_children_elements(element);
            let children: Vec<NodeId> = child_elements
                .iter()
                .map(|&e| self.cache_element(e))
                .collect();

            // Determine available actions based on role
            let actions = self.determine_actions(&role);

            Ok(Node {
                id: node_id,
                role,
                name,
                value,
                description,
                bounds: None, // TODO: implement bounds
                actions,
                children,
            })
        }
    }

    fn determine_actions(&self, role: &str) -> Vec<Action> {
        match role {
            "AXButton" => vec![Action::Press, Action::Focus],
            "AXTextField" => vec![
                Action::Focus,
                Action::SetValue {
                    value: String::new(),
                },
            ],
            "AXCheckBox" => vec![Action::Press, Action::Focus],
            "AXSlider" => vec![Action::Focus, Action::Increment, Action::Decrement],
            _ => vec![Action::Focus],
        }
    }
}

impl super::AccessibilityProvider for MacOSProvider {
    fn get_root(&self) -> Result<Node> {
        self.element_to_node(self.root)
    }

    fn get_children(&self, node_id: &NodeId) -> Result<Vec<Node>> {
        let element = self.node_id_to_element(node_id)?;

        unsafe {
            let child_elements = self.get_children_elements(element);
            child_elements
                .iter()
                .map(|&e| self.element_to_node(e))
                .collect()
        }
    }

    fn get_node(&self, node_id: &NodeId) -> Result<Node> {
        let element = self.node_id_to_element(node_id)?;
        self.element_to_node(element)
    }

    fn perform_action(&self, node_id: &NodeId, action: &Action) -> Result<()> {
        let element = self.node_id_to_element(node_id)?;

        let action_name = match action {
            Action::Press => "AXPress",
            Action::Focus => "AXRaise",
            Action::Increment => "AXIncrement",
            Action::Decrement => "AXDecrement",
            _ => anyhow::bail!("Action not yet implemented: {:?}", action),
        };

        unsafe {
            let cf_action = CFString::new(action_name);
            let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());

            if result == K_AX_ERROR_SUCCESS {
                Ok(())
            } else {
                anyhow::bail!("Failed to perform action: error code {}", result)
            }
        }
    }
}

unsafe impl Send for MacOSProvider {}
unsafe impl Sync for MacOSProvider {}
