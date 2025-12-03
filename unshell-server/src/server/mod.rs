mod database;
mod manager;

use std::sync::{Arc, Mutex};

use unshell_lib::module::Manager;

use crate::SERVER_CONFIG;

#[derive(Clone)]
pub struct Server {
    pub manager: Arc<Mutex<Manager>>,
    pub db: sled::Db,
}

impl Server {
    pub fn new(database: String) -> Self {
        Self {
            manager: Manager::start(&SERVER_CONFIG, Vec::new()),
            db: sled::open(database).expect("Failed to open database"),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.db.flush().expect("Failed to flush database on drop");
        Manager::join(self.manager.clone());
    }
}
