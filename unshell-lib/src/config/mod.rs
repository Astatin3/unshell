pub mod config_struct;
mod tree;

pub use tree::{InterfaceData, InterfaceStruct, Tree, TreeMessage};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub parent_component: String,
    pub name: String,
    pub config: HashMap<String, String>,
}
