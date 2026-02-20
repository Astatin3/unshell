//! Configuration structure handling.
//!
//! Provides a struct-based configuration system that maintains
//! both the schema (keys) and values for component configuration.
//!
//! # Usage
//!
//! ```rust
//! use unshell::config::{ConfigStructField, Config};
//!
//! let keys = vec![
//!     ConfigStructField::String { default: "value".to_string(), max_length: Some(100), protected: false },
//!     ConfigStructField::Integer { default: 42, min: Some(0), max: Some(100) },
//! ];
//!
//! let mut config = Config::new(keys);
//! ```

use serde_json::{json, Value};

use crate::{
    config::{ConfigStructField, InterfaceData, InterfaceStruct, TreeMessage},
    warn, ModuleError, Result,
};

/// Type alias for configuration field definitions
pub type ConfigStructKeys = Vec<ConfigStructField>;

/// Type alias for configuration values
pub type ConfigStructValues = Vec<Value>;

/// Configuration container holding both schema and values.
///
/// Manages a structured configuration with typed fields and their values.
/// Supports serialization and tree-based message handling.
pub struct Config {
    keys: ConfigStructKeys,
    values: ConfigStructValues,
}

impl Config {
    /// Create a new Config with given field definitions.
    ///
    /// Values are initialized to defaults from the field definitions.
    pub fn new(keys: ConfigStructKeys) -> Self {
        let values = keys
            .iter()
            .map(|key| match key {
                ConfigStructField::Header(_) => Value::Null,
                ConfigStructField::Text(_) => Value::Null,
                ConfigStructField::String { default, .. } => json!(default),
                ConfigStructField::Integer { default, .. } => json!(default),
            })
            .collect();

        Self { keys, values }
    }

    /// Handle tree messages for configuration access.
    ///
    /// # Errors
    ///
    /// Returns an error if the message is not a valid config message
    pub fn get(&mut self, message: TreeMessage) -> Result<TreeMessage> {
        match message {
            TreeMessage::State(InterfaceData::ConfigStruct(values)) => {
                self.values = values;
                Ok(TreeMessage::Success)
            }

            // TreeMessage::RequestStruct => Ok(TreeMessage::Interface(
            //     InterfaceStruct::ConfigStruct(self.keys.clone()),
            // )),
            TreeMessage::RequestState => Ok(TreeMessage::State(InterfaceData::ConfigStruct(
                self.values.clone(),
            ))),
            TreeMessage::RequestStructAndValue => Ok(TreeMessage::InterfaceAndValue(
                InterfaceStruct::ConfigStruct(self.keys.clone()),
                InterfaceData::ConfigStruct(self.values.clone()),
            )),

            _ => {
                warn!("Tree got invalid message");
                Err(ModuleError::Error("Invalid Request".into()))
            }
        }
    }
}
