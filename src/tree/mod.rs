use std::collections::HashMap;

use serde_json::{Value, json};

mod log;
pub mod symbols;

pub trait TreeElement {
    // fn get_children(&self) -> HashMap<TreeType, TreeType>;
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

    // fn send_message_child(&mut self, element: Value, message: TreeMessage) -> TreeMessage {
    //     let name = if let TreeType::String(name) = element {
    //         name
    //     } else {
    //         return TreeMessage::Error(ModuleError::InvalidType);
    //     };

    //     if let Some(element) = self.elements.get_mut(&name) {
    //         element.send_message(message)
    //     } else {
    //         TreeMessage::Error(ModuleError::TreeNotExist)
    //     }
    // }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(message) = message.as_str() {
                    match message {
                        "GetChildren" => {
                            let children = self
                                .elements
                                .iter()
                                .map(|c| (Value::String(c.0.clone()), c.1.get_type()))
                                .into_iter()
                                .collect::<HashMap<Value, Value>>();

                            json!(children)
                        }

                        _ => Value::String("UnsupportedMethod".to_owned()),
                    }
                } else {
                    Value::String("UnsupportedMethod".to_owned())
                }
            }
            Value::String(target) => {
                if let Some(child) = self.elements.get_mut(&target) {
                    child.send_message(Value::Null, message)
                } else {
                    Value::String("UnsupportedMethod".to_owned())
                }
            }
            _ => Value::String("UnsupportedMethod".to_owned()),
        }
    }
}
