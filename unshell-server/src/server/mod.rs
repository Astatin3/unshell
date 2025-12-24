use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use unshell_lib::{
    ModuleError, Result,
    config::{ConfigStructField, Tree, TreeMessage, config_struct::Config},
};
use unshell_manager::Manager;

mod blobs;
mod database;
mod tree2;

#[derive(Clone)]
pub struct Server {
    // pub component_configs: Vec<crate::config::ComponentState>,
    // pub interface: InterfaceWrapper,
    pub manager: Arc<Mutex<Manager>>,
    pub db: sled::Db,
    // pub tree: Tree2,
    test_thing: Arc<Mutex<Config>>,
}

impl Server {
    pub fn new(_config_paths: Vec<PathBuf>, database: String) -> Result<Self> {
        // let mut component_configs: Vec<crate::config::ComponentState> = Vec::new(1);

        // for config in &config_paths {
        //     component_configs.extend(crate::config::load_config(config)?);
        // }

        Ok(Self {
            // component_configs,
            manager: Manager::start(&crate::SERVER_CONFIG, Vec::new()),
            db: sled::open(database).map_err(|e| ModuleError::DatabaseError(e.to_string()))?,

            test_thing: Arc::new(Mutex::new(Config::new(vec![
                ConfigStructField::Header("Test Heading".into()),
                ConfigStructField::Text("Test Texttttttttttttttt".into()),
                ConfigStructField::String {
                    default: "Test Texttttttttttttttt".into(),
                    max_length: None,
                    protected: true,
                },
                ConfigStructField::String {
                    default: "Test ".into(),
                    max_length: Some(15),
                    protected: false,
                },
            ]))),
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

    fn select_child(&mut self, child: &str, message: TreeMessage) -> Result<TreeMessage> {
        match child {
            "connection_count" => self.test_thing.lock().unwrap().get(message),
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
