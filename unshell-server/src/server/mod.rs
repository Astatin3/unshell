use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use unshell_lib::{
    ModuleError,
    config::{InterfaceData, Tree, TreeMessage, config_struct::ConfigStructField},
};
use unshell_lib::{Result, config::InterfaceStruct};
use unshell_manager::Manager;

mod blobs;
mod database;
mod tree2;

#[derive(Clone)]
pub struct Server {
    pub component_configs: Vec<crate::config::ComponentState>,
    // pub interface: InterfaceWrapper,
    pub manager: Arc<Mutex<Manager>>,
    pub db: sled::Db,
    // pub tree: Tree2,
}

impl Server {
    pub fn new(config_paths: Vec<PathBuf>, database: String) -> Result<Self> {
        let mut component_configs: Vec<crate::config::ComponentState> = Vec::new();

        for config in &config_paths {
            component_configs.extend(crate::config::load_config(config)?);
        }

        Ok(Self {
            component_configs,
            manager: Manager::start(&crate::SERVER_CONFIG, Vec::new()),
            db: sled::open(database).map_err(|e| ModuleError::DatabaseError(e.to_string()))?,
            // tree: Tree2::default(),
            // interface: get_test_interface(),
        })
    }
}

impl Tree for Server {
    fn is_folder() -> bool {
        true
    }

    fn get_children_string(&self) -> Vec<String> {
        vec!["connection_count".into()]
    }

    fn select_child(&self, child: &str, _message: TreeMessage) -> Result<TreeMessage> {
        match child {
            "connection_count" => {
                let interface = vec![ConfigStructField::Header(format!("Test Heading!"))];

                let value = vec![Value::Null];

                Ok(TreeMessage::InterfaceAndValue(
                    InterfaceStruct::ConfigStruct(interface),
                    InterfaceData::ConfigStruct(value),
                ))
            }
            _ => Err("No such child".into()),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.db.flush().expect("Failed to flush database on drop");
        // Manager::join(self.manager.clone());
    }
}
