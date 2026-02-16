//! Component system for extensible modular architecture.
//!
//! Components are TreeElements that can be dynamically added to endpoints
//! and expose configuration and RPC methods.

use serde_json::{json, Value};

use crate::tree::{Branch, TreeElement};
use crate::{error, info, warn};

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

    fn handle_rpc(&mut self, payload: &Value) -> Value {
        let method = match payload.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => return json!({"success": false, "error": "missing method"}),
        };

        match method {
            "status" => self.component.status(),
            "shutdown" => match self.component.shutdown() {
                Ok(_) => json!({"success": true}),
                Err(e) => json!({"success": false, "error": e}),
            },
            _ => json!({"success": false, "error": format!("unknown method: {}", method)}),
        }
    }
}

impl TreeElement for ComponentWrapper {
    fn get_type(&self) -> Value {
        json!(["component", self.component.name()])
    }

    fn send_message(&mut self, _target: Value, message: Value) -> Value {
        // Handle RPC call format
        if let Some(obj) = message.as_object() {
            if let Some(_) = obj.get("method") {
                return self.handle_rpc(&message);
            }
        }

        // Legacy string commands
        if let Some(cmd) = message.as_str() {
            match cmd {
                "Status" => self.component.status(),
                "Init" => json!({"error": "Init requires config payload"}),
                "Shutdown" => match self.component.shutdown() {
                    Ok(_) => json!({"success": true}),
                    Err(e) => json!({"success": false, "error": e}),
                },
                "GetChildren" => json!([self.component.name()]),
                _ => json!({"error": "Unknown command"}),
            }
        } else {
            json!({"error": "Invalid command"})
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

    pub fn register(&mut self, component: Box<dyn Component>) -> Result<(), String> {
        let name = component.name().to_string();

        if self.branch.get_child(&name).is_some() {
            warn!("Component '{}' already registered", name);
            return Err(format!("Component '{}' already registered", name));
        }

        let wrapper = ComponentWrapper::new(component);
        self.branch.add_child(name.clone(), Box::new(wrapper));
        info!("Component '{}' registered successfully", name);
        Ok(())
    }

    pub fn register_element(&mut self, name: impl Into<String>, element: Box<dyn TreeElement>) {
        self.branch.add_child(name.into(), element);
    }

    pub fn get(&mut self, name: &str) -> Option<&mut Box<dyn TreeElement>> {
        self.branch.get_child(name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.branch.children().contains_key(name)
    }

    /// Remove a component from the registry by name.
    /// Returns true if the component was found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let removed = self.branch.remove_child(name).is_some();
        if removed {
            info!("Component '{}' removed successfully", name);
        } else {
            warn!("Component '{}' not found for removal", name);
        }
        removed
    }

    pub fn list(&self) -> Vec<String> {
        self.branch.children().keys().cloned().collect()
    }

    pub fn branch(&self) -> &Branch {
        &self.branch
    }

    pub fn branch_mut(&mut self) -> &mut Branch {
        &mut self.branch
    }

    pub fn send_to_component(&mut self, component_name: &str, message: Value) -> Value {
        if let Some(component) = self.branch.get_child(component_name) {
            component.send_message(json!(null), message)
        } else {
            warn!("Component '{}' not found", component_name);
            let err_msg = format!("Component '{}' not found", component_name);
            json!({"error": err_msg})
        }
    }

    pub fn broadcast(&mut self, message: Value) -> Vec<(String, Value)> {
        let names: Vec<String> = self.branch.children().keys().cloned().collect();

        names
            .iter()
            .filter_map(|name| {
                if let Some(component) = self.branch.get_child(name) {
                    Some((
                        name.clone(),
                        component.send_message(json!(null), message.clone()),
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Shutdown all registered components gracefully.
    /// Returns a list of component names and their shutdown results.
    pub fn shutdown_all(&mut self) -> Vec<(String, Result<(), String>)> {
        info!("Shutting down all components");
        let names: Vec<String> = self.branch.children().keys().cloned().collect();

        let results: Vec<(String, Result<(), String>)> = names
            .into_iter()
            .filter_map(|name| {
                // Try to send shutdown message to each component
                if let Some(component) = self.branch.get_child(&name) {
                    let result = component.send_message(json!(null), json!({"method": "shutdown"}));

                    // Check if shutdown was successful
                    let success = result
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let err = if success {
                        info!("Component '{}' shutdown successfully", name);
                        Ok(())
                    } else {
                        let error_msg = result
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        error!("Component '{}' shutdown failed: {}", name, error_msg);
                        Err(error_msg)
                    };

                    Some((name, err))
                } else {
                    None
                }
            })
            .collect();

        info!("All components shutdown complete");
        results
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeElement for ComponentRegistry {
    fn get_type(&self) -> Value {
        json!("Components")
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        // Handle RPC-style component access
        if let Some(target_str) = target.as_str() {
            if target_str.starts_with("rpc.") {
                let component_name = target_str.strip_prefix("rpc.").unwrap_or(target_str);
                return self.send_to_component(component_name, message);
            }
        }

        self.branch.send_message(target, message)
    }
}

/// Helper trait for convenient component registration
pub trait IntoComponent: Component + Sized {
    fn into_boxed(self) -> Box<dyn Component>;
}

impl<T: Component + Sized + 'static> IntoComponent for T {
    fn into_boxed(self) -> Box<dyn Component> {
        Box::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent {
        name: String,
        value: i32,
    }

    impl TestComponent {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                value: 0,
            }
        }
    }

    impl Component for TestComponent {
        fn name(&self) -> &str {
            &self.name
        }

        fn status(&self) -> Value {
            json!({"name": self.name, "value": self.value})
        }

        fn init(&mut self, config: Value) -> Result<(), String> {
            if let Some(v) = config.get("value").and_then(|v| v.as_i64()) {
                self.value = v as i32;
            }
            Ok(())
        }

        fn shutdown(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_component_registry() {
        let mut registry = ComponentRegistry::new();

        let comp = Box::new(TestComponent::new("test"));
        registry.register(comp).unwrap();

        assert!(registry.has("test"));
        assert!(!registry.has("other"));

        let list = registry.list();
        assert_eq!(list, vec!["test"]);
    }

    #[test]
    fn test_rpc_call() {
        let mut registry = ComponentRegistry::new();

        let comp = Box::new(TestComponent::new("test"));
        registry.register(comp).unwrap();

        let result = registry.send_to_component("test", json!({"method": "status"}));

        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("name"));
    }
}
