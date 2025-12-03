use std::collections::HashMap;

use sled::Tree;
use unshell_lib::error;

use crate::server::Server;

// #[derive(Clone)]
// pub struct Database {
//     db: sled::Db,
// }

impl Server {
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

    pub fn all_tree_values(&self, tree_name: &str) -> Result<HashMap<String, String>, String> {
        Ok(self
            .get_keys(tree_name)?
            .iter()
            .map(|key| -> Result<(String, String), String> {
                Ok((key.clone(), self.get_value(tree_name, &key)?))
            })
            .collect::<Result<Vec<(String, String)>, String>>()?
            .into_iter()
            .collect::<HashMap<String, String>>())
    }
}
