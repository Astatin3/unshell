use std::collections::HashMap;

use crate::{
    ModuleError,
    manager::tree_structs::{TreeMessage, TreeType},
};

mod log;
mod tree_structs;

pub trait TreeElement {
    fn get_children(&self) -> HashMap<String, TreeType>;
    fn get_type(&self) -> TreeType;
    fn send_message(&mut self, message: TreeMessage) -> TreeMessage;
    fn send_message_child(&mut self, element: String, message: TreeMessage) -> TreeMessage;
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
    fn get_children(&self) -> HashMap<String, TreeType> {
        self.elements
            .iter()
            .map(|c| (c.0.clone(), c.1.get_type()))
            .into_iter()
            .collect()
    }

    fn get_type(&self) -> TreeType {
        TreeType::RootTree
    }

    fn send_message_child(&mut self, element_name: String, message: TreeMessage) -> TreeMessage {
        if let Some(element) = self.elements.get_mut(&element_name) {
            element.send_message(message)
        } else {
            TreeMessage::Result(ModuleError::TreeNotExist)
        }
    }

    fn send_message(&mut self, _message: TreeMessage) -> TreeMessage {
        TreeMessage::Response
        // if let Some(element) = self.elements.get_mut(&element_name) {
        //     element.send_message(message)
        // } else {
        //     TreeMessage::Result(ModuleError::TreeNotExist)
        // }
    }
}
