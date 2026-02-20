//! Configuration system for unshell components.
//!
//! This module provides types for runtime configuration of components
//! and tree structures.
//!
//! # Overview
//!
//! - `RuntimeConfig`: Configuration for runtime-loaded modules
//! - `ConfigStructField`: Field types for UI/config structures
//! - `Tree`, `TreeMessage`: Tree-based configuration access
//! - `InterfaceData`, `InterfaceStruct`: Data interchange formats
//!
//! # Usage
//!
//! ```rust
//! use unshell::config::{RuntimeConfig, ConfigStructField};
//! use std::collections::HashMap;
//!
//! let config = RuntimeConfig {
//!     parent_component: "root".to_string(),
//!     name: "my_module".to_string(),
//!     config: HashMap::new(),
//! };
//! ```

pub mod config_struct;
// pub mod config_struct_list;
mod tree;

pub use tree::{InterfaceData, InterfaceStruct, Tree, TreeMessage};

use std::collections::HashMap;

/// Configuration for a runtime-loaded module or component.
///
/// This struct holds the information needed to configure
/// and initialize a dynamically loaded component.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// The parent component that loaded this config
    pub parent_component: String,
    /// Unique name for this configuration/module
    pub name: String,
    /// Key-value configuration pairs
    pub config: HashMap<String, String>,
}

/// Field types for structured configuration UI.
///
/// Used to describe the structure of configurable settings
/// in a serializable format suitable for UI rendering or
/// external configuration files.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ConfigStructField {
    /// Section header (non-editable label)
    Header(String),
    /// Multi-line text field
    Text(String),
    /// Single-line string input
    String {
        /// Default value
        #[serde(default)]
        default: String,
        /// Maximum length constraint
        max_length: Option<usize>,
        /// Whether to display as password (masked input)
        #[serde(default)]
        protected: bool,
    },
    /// Integer input with bounds
    Integer {
        /// Default value
        #[serde(default)]
        default: i32,
        /// Minimum allowed value
        min: Option<i32>,
        /// Maximum allowed value
        max: Option<i32>,
    },
    // Checkbox
    // Dropdown
    // Collapsing header
    // Slider
    // ...
}
