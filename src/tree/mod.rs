use std::collections::HashMap;

use serde_json::{json, Value};

mod branch;
pub mod log;
pub mod symbols;

pub use branch::Branch;

pub trait TreeElement {
    fn get_type(&self) -> Value;
    fn send_message(&mut self, target: Value, message: Value) -> Value;
}

pub struct Tree {
    elements: HashMap<String, Box<dyn TreeElement>>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }

    pub fn add_element(&mut self, name: String, element: Box<dyn TreeElement>) {
        self.elements.insert(name, element);
    }
}

impl TreeElement for Tree {
    fn get_type(&self) -> Value {
        json!(symbols::TYPE_TREE)
    }
    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(message) = message.as_str() {
                    match message {
                        symbols::CMD_GET_CHILDREN => {
                            let children = self
                                .elements
                                .iter()
                                .map(|(k, v)| (Value::String(k.clone()), v.get_type()))
                                .collect::<HashMap<Value, Value>>();
                            json!(children)
                        }
                        _ => json!(symbols::ERR_UNSUPPORTED_METHOD),
                    }
                } else {
                    json!(symbols::ERR_UNSUPPORTED_METHOD)
                }
            }
            Value::Array(mut path) => {
                if path.is_empty() {
                    return json!(symbols::ERR_INVALID_PATH);
                }
                let next = path.remove(0);
                if let Value::String(next_name) = next {
                    if let Some(child) = self.elements.get_mut(&next_name) {
                        child.send_message(Value::Array(path), message)
                    } else {
                        json!(symbols::ERR_CHILD_NOT_FOUND)
                    }
                } else {
                    json!(symbols::ERR_INVALID_PATH)
                }
            }
            Value::String(target) => {
                if let Some(child) = self.elements.get_mut(&target) {
                    child.send_message(Value::Null, message)
                } else {
                    json!(symbols::ERR_CHILD_NOT_FOUND)
                }
            }
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}
