use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type ConfigStructKeys = Vec<ConfigStructField>;
pub type ConfigStructValues = Vec<Value>;

pub struct Config {
    keys: ConfigStructKeys,
    values: ConfigStructValues,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
