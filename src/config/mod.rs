pub mod config_struct;
// pub mod config_struct_list;
mod tree;

pub use tree::{InterfaceData, InterfaceStruct, Tree, TreeMessage};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub parent_component: String,
    pub name: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ConfigStructField {
    Header(String),
    Text(String),
    String {
        // Default value of string edit in struct
        #[serde(default)]
        default: String,
        max_length: Option<usize>,
        // Display string edit as password
        #[serde(default)]
        protected: bool,
    },
    Integer {
        // Default value of integer in struct
        #[serde(default)]
        default: i32,
        min: Option<i32>,
        max: Option<i32>,
    },
    // Checkbox
    // Dropdown
    // Collapsing header
    // Slider
    // ...
}
