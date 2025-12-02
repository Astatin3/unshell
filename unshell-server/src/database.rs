use sled::Tree;
use unshell_lib::error;

#[derive(Clone)]
pub struct Database {
    db: sled::Db,
}

impl Database {
    pub fn new(database: String) -> Self {
        Self {
            db: sled::open(database).expect("Failed to open database"),
        }
    }

    fn get_tree(&self, tree_name: &str) -> Result<Tree, String> {
        self.db.open_tree(tree_name).map_err(|e| {
            error!("DB Failed to open tree: {}", e);
            "Internal server error".to_string()
        })
    }

    pub fn get_trees(&self) -> Vec<String> {
        self.db
            .tree_names()
            .iter()
            .map(|n| String::from_utf8_lossy(&n.to_vec()).to_string())
            .collect::<Vec<String>>()
    }

    pub fn get_keys(&self, tree_name: &str) -> Result<Vec<String>, String> {
        Ok(self
            .get_tree(tree_name)?
            .iter()
            .keys()
            .map(|key| {
                String::from_utf8_lossy(&key.expect("This key should exist").to_vec()).to_string()
            })
            .collect::<Vec<String>>())
    }

    pub fn put_value(&self, tree_name: &str, key: &str, value: &str) -> Result<(), String> {
        match self.get_tree(tree_name)?.insert(key, value) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to load '{}' from database: {}", key, e);
                Err("Internal server error".to_string())
            }
        }
    }

    pub fn get_value(&self, tree_name: &str, key: &str) -> Result<String, String> {
        match self.get_tree(tree_name)?.get(key) {
            Ok(v) => match v {
                Some(v) => Ok(String::from_utf8_lossy(&v.to_vec()).to_string()),
                None => Err(format!("Could not find key '{}'", key)),
            },
            Err(e) => {
                error!("Failed to load '{}' from database: {}", key, e);
                Err("Internal server error".to_string())
                // Err(e.to_string())
            }
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        self.db.flush().expect("Failed to flush database on drop");
    }
}
