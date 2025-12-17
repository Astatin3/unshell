use std::collections::HashMap;

use serde_json::Value;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub enum Tree2Repr {
    File(String),
    Folder(Vec<String>),
}

// #[derive(Clone, serde::Deserialize, serde::Serialize)]
// pub struct InterfaceWrapper {
//     pub name: String,
//     pub interface: Interface,
// }

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
        protected: Option<bool>,
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
