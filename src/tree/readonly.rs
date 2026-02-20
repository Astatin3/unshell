//! TreeVariable - A TreeElement with getters and setters.
//!
//! ReadOnly - A wrapper around TreeVariable that ignores setters.
//!
//! # Usage
//!
//! ## TreeVariable
//!
//! ```rust
//! use unshell::tree::{TreeVariable, TreeElement};
//! use serde_json::json;
//!
//! let mut var = TreeVariable::new("default_value", "String");
//!
//! // Get value via tree message
//! let result = var.send_message(json!(null), json!("Get"));
//! assert_eq!(result, json!("default_value"));
//!
//! // Set value via tree message
//! let result = var.send_message(json!("Set"), json!("new_value"));
//! assert_eq!(result, json!(true));
//! ```
//!
//! ## ReadOnly
//!
//! ```rust
//! use unshell::tree::ReadOnly;
//! use serde_json::json;
//!
//! let mut var = ReadOnly::new("immutable", "String");
//!
//! // Get works
//! let result = var.send_message(json!(null), json!("Get"));
//! assert_eq!(result, json!("immutable"));
//!
//! // Set returns ReadOnly error
//! let result = var.send_message(json!("Set"), json!("new_value"));
//! ```

use serde_json::{json, Value};

use crate::tree::symbols;
use crate::tree::TreeElement;

/// A variable with getters and setters exposed through the tree.
///
/// Supports:
/// - `Get`: Retrieve current value
/// - `Set`: Set new value (requires string message)
pub struct TreeVariable {
    value: String,
    value_type: &'static str,
}

impl TreeVariable {
    /// Create a new tree variable.
    pub fn new(value: impl Into<String>, value_type: &'static str) -> Self {
        Self {
            value: value.into(),
            value_type,
        }
    }

    /// Get the current value.
    pub fn get(&self) -> &str {
        &self.value
    }

    /// Set a new value.
    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }
}

impl TreeElement for TreeVariable {
    fn get_type(&self) -> Value {
        json!(self.value_type)
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        symbols::CMD_GET => json!(self.value.clone()),
                        "Set" => json!(symbols::ERR_MISSING_ARGS),
                        _ => json!(symbols::ERR_UNSUPPORTED_METHOD),
                    }
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            Value::String(cmd) if cmd == "Set" => {
                if let Some(new_value) = message.as_str() {
                    self.value = new_value.to_string();
                    json!(true)
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}

/// A read-only wrapper around TreeVariable that ignores setters.
///
/// Any attempt to set the value returns an error.
/// Useful for exposing immutable endpoint identifiers.
pub struct ReadOnly {
    inner: TreeVariable,
}

impl ReadOnly {
    /// Create a new read-only variable.
    pub fn new(value: impl Into<String>, value_type: &'static str) -> Self {
        Self {
            inner: TreeVariable::new(value, value_type),
        }
    }

    /// Get the current value.
    pub fn get(&self) -> &str {
        self.inner.get()
    }

    /// Get immutable reference to inner TreeVariable.
    pub fn inner(&self) -> &TreeVariable {
        &self.inner
    }

    /// Get mutable reference to inner TreeVariable.
    pub fn inner_mut(&mut self) -> &mut TreeVariable {
        &mut self.inner
    }
}

impl TreeElement for ReadOnly {
    fn get_type(&self) -> Value {
        self.inner.get_type()
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        symbols::CMD_GET => json!(self.inner.get()),
                        "Set" => json!(symbols::ERR_READONLY),
                        _ => json!(symbols::ERR_UNSUPPORTED_METHOD),
                    }
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            Value::String(cmd) if cmd == "Set" => json!(symbols::ERR_READONLY),
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}
