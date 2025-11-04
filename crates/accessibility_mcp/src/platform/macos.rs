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
const K_AX_POSITION_ATTRIBUTE: &str = "AXPosition";
const K_AX_SIZE_ATTRIBUTE: &str = "AXSize";

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

    /// Get a point attribute (position) from an AX element
    unsafe fn get_point_attribute(
        &self,
        element: AXUIElementRef,
        attr: &str,
    ) -> Option<(f64, f64)> {
        use core_foundation::base::TCFType;

        let attr_name = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null();

        let result =
            AXUIElementCopyAttributeValue(element, attr_name.as_concrete_TypeRef(), &mut value);

        if result != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        // The value is an AXValue containing a CGPoint
        // We need to extract x and y coordinates
        // For simplicity, try to get it as a CFType and inspect it
        let _cf_value = CFType::wrap_under_create_rule(value);

        // AXValue is a CFType but not directly exposed in core-foundation
        // We'll use a workaround: extract the raw bytes
        // CGPoint is {CGFloat x; CGFloat y;} where CGFloat is f64 on 64-bit systems
        #[repr(C)]
        struct CGPoint {
            x: f64,
            y: f64,
        }

        // Use AXValueGetValue to extract the point
        extern "C" {
            fn AXValueGetValue(
                value: CFTypeRef,
                type_: i32,
                value_ptr: *mut std::ffi::c_void,
            ) -> bool;
        }

        const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;

        let mut point = CGPoint { x: 0.0, y: 0.0 };
        let success = AXValueGetValue(
            value,
            K_AX_VALUE_CG_POINT_TYPE,
            &mut point as *mut _ as *mut std::ffi::c_void,
        );

        if success {
            // Convert from macOS coordinates (bottom-left origin) to screen coordinates (top-left origin)
            // We need the screen height to do this conversion
            // For now, return raw coordinates - caller can convert if needed
            Some((point.x, point.y))
        } else {
            None
        }
    }

    /// Get a size attribute from an AX element
    unsafe fn get_size_attribute(&self, element: AXUIElementRef, attr: &str) -> Option<(f64, f64)> {
        use core_foundation::base::TCFType;

        let attr_name = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null();

        let result =
            AXUIElementCopyAttributeValue(element, attr_name.as_concrete_TypeRef(), &mut value);

        if result != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        let _cf_value = CFType::wrap_under_create_rule(value);

        #[repr(C)]
        struct CGSize {
            width: f64,
            height: f64,
        }

        extern "C" {
            fn AXValueGetValue(
                value: CFTypeRef,
                type_: i32,
                value_ptr: *mut std::ffi::c_void,
            ) -> bool;
        }

        const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;

        let mut size = CGSize {
            width: 0.0,
            height: 0.0,
        };
        let success = AXValueGetValue(
            value,
            K_AX_VALUE_CG_SIZE_TYPE,
            &mut size as *mut _ as *mut std::ffi::c_void,
        );

        if success {
            Some((size.width, size.height))
        } else {
            None
        }
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

            // Get bounds (position and size)
            let bounds = if let (Some((x, y)), Some((width, height))) = (
                self.get_point_attribute(element, K_AX_POSITION_ATTRIBUTE),
                self.get_size_attribute(element, K_AX_SIZE_ATTRIBUTE),
            ) {
                Some(crate::protocol::Rect {
                    x,
                    y,
                    width,
                    height,
                })
            } else {
                None
            };

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
                bounds,
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

        match action {
            Action::Press => unsafe {
                let cf_action = CFString::new("AXPress");
                let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());
                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to perform press action: error code {}", result)
                }
            },
            Action::Focus => unsafe {
                let cf_action = CFString::new("AXRaise");
                let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());
                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to perform focus action: error code {}", result)
                }
            },
            Action::Increment => unsafe {
                let cf_action = CFString::new("AXIncrement");
                let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());
                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to perform increment action: error code {}", result)
                }
            },
            Action::Decrement => unsafe {
                let cf_action = CFString::new("AXDecrement");
                let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());
                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to perform decrement action: error code {}", result)
                }
            },
            Action::SetValue { value } => unsafe {
                let attr_name = CFString::new(K_AX_VALUE_ATTRIBUTE);
                let cf_value = CFString::new(value);

                extern "C" {
                    fn AXUIElementSetAttributeValue(
                        element: AXUIElementRef,
                        attribute: CFStringRef,
                        value: CFTypeRef,
                    ) -> AXError;
                }

                let result = AXUIElementSetAttributeValue(
                    element,
                    attr_name.as_concrete_TypeRef(),
                    cf_value.as_CFTypeRef(),
                );

                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to set value: error code {}", result)
                }
            },
            Action::Scroll { x: _, y: _ } => {
                // Scroll is not directly supported by AX API in the same way
                // It would require finding scroll bars and incrementing/decrementing them
                // or using AXScrollToVisible action
                anyhow::bail!("Scroll action not yet implemented for macOS")
            }
            Action::ContextMenu => unsafe {
                let cf_action = CFString::new("AXShowMenu");
                let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());
                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to show context menu: error code {}", result)
                }
            },
            Action::Custom { name } => unsafe {
                let cf_action = CFString::new(name);
                let result = AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef());
                if result == K_AX_ERROR_SUCCESS {
                    Ok(())
                } else {
                    anyhow::bail!(
                        "Failed to perform custom action '{}': error code {}",
                        name,
                        result
                    )
                }
            },
        }
    }
}

unsafe impl Send for MacOSProvider {}
unsafe impl Sync for MacOSProvider {}
