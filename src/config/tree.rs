//! Tree-based configuration access.
//!
//! Provides a hierarchical tree structure for configuration access,
//! similar to a file system with folders and values.
//!
//! # Architecture
//!
//! - `Tree` trait: Implement for any config container
//! - `TreeMessage`: Request/response messages
//! - `InterfaceStruct`: Schema definitions
//! - `InterfaceData`: Data values
//!
//! # Usage
//!
//! ```rust
//! use unshell::config::{Tree, TreeMessage};
//!
//! struct MyConfig;
//!
//! impl Tree for MyConfig {
//!     fn select_child(&mut self, child: &str, message: TreeMessage) -> Result<TreeMessage> {
//!         // Handle child selection
//!         Ok(TreeMessage::Success)
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::{config::config_struct, ModuleError, Result};

/// Trait for tree-structured configuration.
///
/// Implement this trait to provide hierarchical configuration access,
/// similar to a file system with folders (containers) and files (values).
pub trait Tree {
    /// Check if this node is a folder (container) vs leaf (value)
    fn is_folder() -> bool {
        false
    }

    /// Get list of child names (for folders)
    fn get_children_string(&self) -> Vec<String> {
        unimplemented!();
    }

    /// Select and process a child node
    fn select_child(&mut self, child: &str, _message: TreeMessage) -> Result<TreeMessage>;

    /// Get value at this node
    fn get_value(&self, _message: TreeMessage) -> TreeMessage {
        unimplemented!()
    }

    /// Navigate through path elements
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

    /// Get value at specified path
    ///
    /// # Errors
    ///
    /// Returns error if path is invalid or node not found
    fn get(&mut self, path: &str, message: TreeMessage) -> Result<TreeMessage> {
        let mut path = if path.is_empty() {
            Vec::new()
        } else {
            path.split('/').collect::<Vec<&str>>()
        };

        self.get_path(&mut path, message)
    }
}

/// Messages for tree-based configuration access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TreeMessage {
    /// Request current state/value
    RequestState,
    /// Request structure/schema
    RequestStruct,
    /// Request both structure and current value
    RequestStructAndValue,

    /// Response containing data
    State(InterfaceData),
    // Interface(InterfaceStruct),
    /// Response with both schema and data
    InterfaceAndValue(InterfaceStruct, InterfaceData),

    /// Operation succeeded
    Success,
    /// Operation failed
    Failure,

    /// Folder response (list of children)
    Folder(Vec<String>),
}

/// Schema/structure definitions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InterfaceStruct {
    /// Configuration structure with field definitions
    ConfigStruct(config_struct::ConfigStructKeys),
}

/// Data values for interfaces.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InterfaceData {
    /// Configuration values
    ConfigStruct(config_struct::ConfigStructValues),
}
