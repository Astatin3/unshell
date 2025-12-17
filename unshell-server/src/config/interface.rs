use std::collections::HashMap;

use serde_json::Value;

use crate::config::ConfigStructField;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub enum Interface {
    Sub(HashMap<String, InterfaceWrapper>),
    Struct {
        fields: HashMap<String, ConfigStructField>,
        value: HashMap<String, Value>,
    },
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct InterfaceWrapper {
    pub name: String,
    pub interface: Interface,
}

impl InterfaceWrapper {
    pub fn get_path(&self, elements: &Vec<&str>, depth: usize) -> Result<InterfaceWrapper, String> {
        if depth == elements.len() {
            return Ok(self.clone());
        }

        let element = elements[depth];

        match &self.interface {
            Interface::Sub(interface_wrappers) => {
                if let Some(interface) = interface_wrappers.get(element) {
                    interface.get_path(elements, depth)
                } else {
                    Err("Invalid Path".into())
                }
            }

            _ => Err("Invalid Path".into()),
        }
    }
}

pub fn get_test_interface() -> InterfaceWrapper {
    InterfaceWrapper {
        name: "Root Interface".into(),
        interface: Interface::Sub(HashMap::new()),
    }
}
