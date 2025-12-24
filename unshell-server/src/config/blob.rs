use std::collections::HashMap;

use unshell_lib::config::ConfigStructField;

// use crate::config::ConfigStructField;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Blob {
    name: String,

    parent_component: String,
    // parent_runtime: String,
    config: HashMap<String, ConfigStructField>,
}
