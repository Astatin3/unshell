//! Branch - A TreeElement with child elements for hierarchical routing.

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
