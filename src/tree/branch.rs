//! Branch - A TreeElement with child elements for hierarchical routing.
//!
//! A Branch is a container node in the tree hierarchy that can hold multiple
//! child elements. It provides path-based message routing to traverse the
//! tree structure.
//!
//! # Path-Based Routing
//!
//! Messages can target elements using path notation:
//!
//! ```rust
//! use serde_json::json;
//! use unshell::tree::{Branch, TreeElement};
//!
//! let mut branch = Branch::new("parent");
//! // ... add children ...
//!
//! // Target a direct child
//! branch.send_message(json!("child-name"), json!("Command"));
//!
//! // Target a nested child using array path
//! branch.send_message(json!(["parent", "child", "grandchild"]), json!("Command"));
//! ```
//!
//! # Child Management
//!
//! ```rust
//! use unshell::tree::{Branch, TreeElement};
//! use serde_json::json;
//!
//! let mut branch = Branch::new("my-branch");
//!
//! // Add children
//! branch.add_child("child1", Box::new(ChildElement));
//! branch.add_child("child2", Box::new(AnotherElement));
//!
//! // Query children
//! let children = branch.send_message(json!(null), json!("GetChildren"));
//! // Returns: {"child1": "ChildElement", "child2": "AnotherElement"}
//! ```
//!
//! # Pivot Routing
//!
//! Branches support multi-hop communication for pivoting through networks.
//! A path like `["endpoint1", "connections", "peer1", "endpoint2"]` would
//! route a message through multiple endpoints to reach a final destination.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::tree::symbols;
use crate::tree::TreeElement;

/// A branch node in the tree that can contain child elements.
/// Supports path-based routing for multi-hop communication (pivoting).
pub struct Branch {
    children: HashMap<String, Box<dyn TreeElement>>,
    branch_type: &'static str,
}

impl Default for Branch {
    fn default() -> Self {
        Self::new("default")
    }
}

impl std::fmt::Debug for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Branch")
            .field("branch_type", &self.branch_type)
            .field("children", &self.children.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Branch {
    pub fn new(branch_type: &'static str) -> Self {
        Self {
            children: HashMap::new(),
            branch_type,
        }
    }

    pub fn add_child(&mut self, name: impl Into<String>, child: Box<dyn TreeElement>) {
        self.children.insert(name.into(), child);
    }

    pub fn with_child(mut self, name: impl Into<String>, child: Box<dyn TreeElement>) -> Self {
        self.add_child(name, child);
        self
    }

    pub fn get_child(&mut self, name: &str) -> Option<&mut Box<dyn TreeElement>> {
        self.children.get_mut(name)
    }

    pub fn children(&self) -> &HashMap<String, Box<dyn TreeElement>> {
        &self.children
    }

    pub fn get_type(&self) -> Value {
        json!(self.branch_type)
    }

    /// Remove a child by name, returning the removed element
    pub fn remove_child(&mut self, name: &str) -> Option<Box<dyn TreeElement>> {
        self.children.remove(name)
    }

    /// Get a reference to the children map (for iteration)
    pub fn children_mut(&mut self) -> &mut HashMap<String, Box<dyn TreeElement>> {
        &mut self.children
    }

    pub fn send_message(&mut self, target: Value, message: Value) -> Value {
        self.handle_local_message(target, message)
    }

    fn handle_local_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        symbols::CMD_GET_CHILDREN => {
                            let children = self
                                .children
                                .iter()
                                .map(|(k, v)| (Value::String(k.clone()), v.get_type()))
                                .collect::<HashMap<Value, Value>>();
                            json!(children)
                        }
                        _ => self.handle_message(message),
                    }
                } else {
                    self.handle_message(message)
                }
            }
            Value::Array(mut path) => {
                if path.is_empty() {
                    return json!(symbols::ERR_INVALID_PATH);
                }
                let next = path.remove(0);
                if let Value::String(next_name) = next {
                    if let Some(child) = self.children.get_mut(&next_name) {
                        child.send_message(Value::Array(path), message)
                    } else {
                        json!(symbols::ERR_CHILD_NOT_FOUND)
                    }
                } else {
                    json!(symbols::ERR_INVALID_PATH)
                }
            }
            Value::String(target) => {
                if let Some(child) = self.children.get_mut(&target) {
                    child.send_message(Value::Null, message)
                } else {
                    json!(symbols::ERR_CHILD_NOT_FOUND)
                }
            }
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }

    pub fn handle_message(&mut self, _message: Value) -> Value {
        json!(symbols::ERR_UNSUPPORTED_METHOD)
    }
}

impl TreeElement for Branch {
    fn get_type(&self) -> Value {
        self.get_type()
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        self.handle_local_message(target, message)
    }
}
