//! List-based configuration structure.
//!
//! Provides configuration for lists/arrays of structured data.
//! Currently unused - maintained for potential future use.
//!
//! # Overview
//!
//! Similar to `Config` but supports multiple rows of configuration.
//! Useful for configuring lists of items like connection profiles.

use serde_json::Value;

use crate::config::ConfigStructField;

/// Type alias for list configuration keys
pub type ConfigStructListKeys = Vec<ConfigStructField>;

/// Type alias for list configuration values (multiple rows)
pub type ConfigStructListValues = Vec<Vec<Value>>;

/// Configuration container for list-based settings.
///
/// Currently unimplemented - placeholder for future
/// multi-row configuration support.
pub struct ConfigStructList {
    keys: ConfigStructListKeys,
    values: ConfigStructListValues,
}

impl ConfigStructList {
    /// Create a new ConfigStructList with given keys.
    pub fn new(keys: ConfigStructListKeys) -> Self {
        // let values = keys
        //     .iter()
        //     .map(|key| match key {
        //         ConfigStructField::Header(_) => Value::Null,
        //         ConfigStructField::Text(_) => Value::Null,
        //         ConfigStructField::String { default, .. } => json!(default),
        //         ConfigStructField::Integer { default, .. } => json!(default),
        //     })
        //     .collect();

        Self {
            keys,
            values: Vec::new(),
        }
    }
}

// impl Tree for ConfigStructList {}
