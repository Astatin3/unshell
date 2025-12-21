use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde_json::Value;
use sled::Tree;
use unshell_lib::{debug, error};

use crate::{auth::structs::CurrentUser, server::Server};

impl Server {
    fn get_tree(&self, tree_name: &str) -> Result<Tree, String> {
        self.db.open_tree(tree_name).map_err(|e| {
            error!("DB Failed to open tree: {}", e);
            "Internal server error".to_string()
        })
    }

    pub async fn get_trees_api(State(server): State<Server>) -> Json<Value> {
        debug!("GET tree list");

        let result = server
            .db
            .tree_names()
            .iter()
            .map(|n| String::from_utf8_lossy(&n.to_vec()).to_string())
            .collect::<Vec<String>>();

        Json(serde_json::to_value(result).unwrap())
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

    fn get_keys(&self, tree_name: &str) -> Result<Vec<String>, String> {
        Ok(self
            .get_tree(tree_name)?
            .iter()
            .keys()
            .map(|key| {
                String::from_utf8_lossy(&key.expect("This key should exist").to_vec()).to_string()
            })
            .collect::<Vec<String>>())
    }

    // Route the "keys" api for each tree
    pub async fn all_tree_keys_api(
        State(server): State<Server>,
        Path(tree_name): Path<String>,
        Extension(_): Extension<CurrentUser>,
    ) -> Json<Value> {
        let result = server.get_keys(&tree_name);

        Json(serde_json::to_value(result).unwrap())
    }

    // Route the "values" api to get all the values for each tree
    pub async fn all_tree_values_api(
        State(server): State<Server>,
        Path(tree_name): Path<String>,
        Extension(_): Extension<CurrentUser>,
    ) -> Json<Value> {
        let result = || -> Result<HashMap<String, String>, String> {
            Ok(server
                .get_keys(&tree_name)?
                .iter()
                .map(|key| -> Result<(String, String), String> {
                    Ok((key.clone(), server.get_value(&tree_name, &key)?))
                })
                .collect::<Result<Vec<(String, String)>, String>>()?
                .into_iter()
                .collect::<HashMap<String, String>>())
        }();

        Json(serde_json::to_value(result).unwrap())
    }
}
