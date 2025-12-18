use std::{
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use unshell_lib::module::Manager;

use crate::server::tree2::{Tree, Tree2Repr};

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
    pub fn new(config_paths: Vec<PathBuf>, database: String) -> Result<Self, Box<dyn Error>> {
        let mut component_configs: Vec<crate::config::ComponentState> = Vec::new();

        for config in &config_paths {
            component_configs.extend(crate::config::load_config(config)?);
        }

        Ok(Self {
            component_configs,
            manager: Manager::start(&crate::SERVER_CONFIG, Vec::new()),
            db: sled::open(database)?,
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

    fn select_child(&self, child: &str) -> Result<Tree2Repr, String> {
        match child {
            "connection_count" => Ok(Tree2Repr::File(format!(
                "Connection count: {}",
                self.manager.lock().unwrap().connections.len()
            ))),
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
