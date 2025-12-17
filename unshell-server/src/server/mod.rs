use std::{error::Error, path::PathBuf};

mod blobs;
mod database;
mod manager;

#[derive(Clone)]
pub struct Server {
    pub component_configs: Vec<crate::config::ComponentState>,
    // pub manager: Arc<Mutex<Manager>>,
    pub db: sled::Db,
}

impl Server {
    pub fn new(config_paths: Vec<PathBuf>, database: String) -> Result<Self, Box<dyn Error>> {
        let mut component_configs: Vec<crate::config::ComponentState> = Vec::new();

        for config in &config_paths {
            component_configs.extend(crate::config::load_config(config)?);
        }

        Ok(Self {
            component_configs,
            // manager: Manager::start(&SERVER_CONFIG, Vec::new()),
            db: sled::open(database)?,
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.db.flush().expect("Failed to flush database on drop");
        // Manager::join(self.manager.clone());
    }
}
