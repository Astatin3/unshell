//! TreeVariable - A TreeElement with getters and setters.
//!
//! ReadOnly - A wrapper around TreeVariable that ignores setters.

use serde_json::{json, Value};

use crate::tree::symbols;
use crate::tree::TreeElement;

/// A variable with getters and setters exposed through the tree.
pub struct TreeVariable {
    value: String,
    value_type: &'static str,
}

impl TreeVariable {
    pub fn new(value: impl Into<String>, value_type: &'static str) -> Self {
        Self {
            value: value.into(),
            value_type,
        }
    }

    pub fn get(&self) -> &str {
        &self.value
    }

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
pub struct ReadOnly {
    inner: TreeVariable,
}

impl ReadOnly {
    pub fn new(value: impl Into<String>, value_type: &'static str) -> Self {
        Self {
            inner: TreeVariable::new(value, value_type),
        }
    }

    pub fn get(&self) -> &str {
        self.inner.get()
    }

    pub fn inner(&self) -> &TreeVariable {
        &self.inner
    }

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
