//! Component system for extensible modular architecture.
//!
//! Components are TreeElements that can be dynamically added to endpoints
//! and expose configuration and RPC methods.

use serde_json::{json, Value};

use crate::tree::{Branch, TreeElement};

/// Trait for component lifecycle management
pub trait Component: Send + Sync {
    /// Get the component's unique name
    fn name(&self) -> &str;

    /// Get component status information
    fn status(&self) -> Value;

    /// Initialize component with configuration
    fn init(&mut self, config: Value) -> Result<(), String>;

    /// Shutdown component gracefully
    fn shutdown(&mut self) -> Result<(), String>;
}

/// Adapter to make any Component work as a TreeElement
pub struct ComponentWrapper {
    component: Box<dyn Component>,
}

impl ComponentWrapper {
    pub fn new(component: Box<dyn Component>) -> Self {
        Self { component }
    }
}

impl TreeElement for ComponentWrapper {
    fn get_type(&self) -> Value {
        serde_json::json!(["component", self.component.name()])
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        "Status" => self.component.status(),
                        "Init" => {
                            // Would need config from message
                            json!({"error": "Init requires config payload"})
                        }
                        "Shutdown" => match self.component.shutdown() {
                            Ok(_) => json!({"success": true}),
                            Err(e) => json!({"success": false, "error": e}),
                        },
                        _ => json!({"error": "Unknown command"}),
                    }
                } else {
                    json!({"error": "Invalid command"})
                }
            }
            _ => json!({"error": "Invalid target"}),
        }
    }
}

/// Component registration and management
pub struct ComponentRegistry {
    branch: Branch,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            branch: Branch::new("Components"),
        }
    }

    /// Register a new component (consumes the component)
    pub fn register(&mut self, component: Box<dyn Component>) -> Result<(), String> {
        let name = component.name().to_string();

        // Check if already exists by trying to get it
        if self.branch.get_child(&name).is_some() {
            return Err(format!("Component '{}' already registered", name));
        }

        let wrapper = ComponentWrapper::new(component);
        self.branch.add_child(name, Box::new(wrapper));
        Ok(())
    }

    /// Get a component by name (via branch)
    pub fn get(&mut self, name: &str) -> Option<&mut Box<dyn TreeElement>> {
        self.branch.get_child(name)
    }

    /// Remove a component
    pub fn remove(&mut self, name: &str) -> bool {
        // Note: This is tricky with current Branch API
        // For now, just return false
        let _ = name;
        false
    }

    /// List all component names
    pub fn list(&self) -> Vec<String> {
        self.branch.children().keys().cloned().collect()
    }

    /// Get the branch for tree integration
    pub fn branch(&self) -> &Branch {
        &self.branch
    }

    /// Get mutable branch for tree integration
    pub fn branch_mut(&mut self) -> &mut Branch {
        &mut self.branch
    }

    /// Shutdown all components
    pub fn shutdown_all(&mut self) {
        // Would need to iterate through and call shutdown on each
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeElement for ComponentRegistry {
    fn get_type(&self) -> Value {
        serde_json::json!("Components")
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        self.branch.send_message(target, message)
    }
}
