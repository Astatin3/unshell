use serde::{Deserialize, Serialize};

use crate::{ModuleError, Result, config::config_struct};

pub trait Tree {
    fn is_folder() -> bool {
        false
    }

    fn get_children_string(&self) -> Vec<String> {
        unimplemented!();
    }

    fn select_child(&mut self, child: &str, _message: TreeMessage) -> Result<TreeMessage>;

    fn get_value(&self, _message: TreeMessage) -> TreeMessage {
        unimplemented!()
    }

    fn get_path(&mut self, elements: &mut Vec<&str>, message: TreeMessage) -> Result<TreeMessage> {
        if elements.is_empty() {
            return if Self::is_folder() {
                Ok(TreeMessage::Folder(self.get_children_string()))
            } else {
                Ok(self.get_value(message))
            };
        }

        let child = elements.remove(0);

        if Self::is_folder() {
            self.select_child(child, message)
        } else {
            Err(ModuleError::TreeMessageError(
                "This is a folder, not a file".into(),
            ))
        }
    }

    fn get(&mut self, path: &str, message: TreeMessage) -> Result<TreeMessage> {
        let mut path = if path.is_empty() {
            Vec::new()
        } else {
            path.split("/").collect::<Vec<&str>>()
        };

        self.get_path(&mut path, message)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TreeMessage {
    RequestState,
    RequestStruct,
    RequestStructAndValue,

    State(InterfaceData),
    Interface(InterfaceStruct),
    InterfaceAndValue(InterfaceStruct, InterfaceData),

    Success,
    Failure,

    Folder(Vec<String>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InterfaceStruct {
    ConfigStruct(config_struct::ConfigStructKeys),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InterfaceData {
    ConfigStruct(config_struct::ConfigStructValues),
}
