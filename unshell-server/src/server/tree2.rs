use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use unshell_lib::debug;

use crate::{Server, api::CurrentUser};

pub trait Tree {
    fn is_folder() -> bool {
        false
    }

    fn get_children_string(&self) -> Vec<String> {
        Vec::new()
    }

    fn select_child(&self, child: &str) -> Result<Tree2Repr, String>;

    fn get_value(&self) -> String {
        "DEFAULT".into()
    }

    fn get_path(&self, elements: &mut Vec<&str>) -> Result<Tree2Repr, String> {
        if elements.is_empty() {
            return if Self::is_folder() {
                Ok(Tree2Repr::Folder(self.get_children_string()))
            } else {
                Ok(Tree2Repr::File(self.get_value()))
            };
        }

        let child = elements.remove(0);

        if Self::is_folder() {
            self.select_child(child)
        } else {
            Err("This is a folder, not a file".into())
        }
    }

    fn get(&self, path: &str) -> Result<Tree2Repr, String> {
        let mut path = if path.is_empty() {
            Vec::new()
        } else {
            path.split("/").collect::<Vec<&str>>()
        };

        self.get_path(&mut path)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Tree2Repr {
    File(String),
    Folder(Vec<String>),
}

impl Server {
    pub async fn get_tree2(
        State(server): State<Server>,
        Path(path): Path<String>,
        Extension(_): Extension<CurrentUser>,
    ) -> Json<Value> {
        debug!("GET /api/interface/{}", path);

        let result = (|| -> Result<_, String> {
            let interface = server.get(&path)?;

            Ok(interface)
        })();

        Json(serde_json::to_value(result).unwrap())
    }
    pub async fn get_tree2_root(
        State(server): State<Server>,
        Extension(extension): Extension<CurrentUser>,
    ) -> Json<Value> {
        Self::get_tree2(State(server), Path("".into()), Extension(extension)).await
    }
}
