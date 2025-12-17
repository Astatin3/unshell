use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use unshell_lib::debug;

use crate::{Server, api::CurrentUser};

#[derive(Clone)]
enum Tree2Branch {
    File(String),
    Folder(String, HashMap<String, Tree2Branch>),
}

#[derive(Clone, Serialize, Deserialize)]
enum Tree2Repr {
    File(String),
    Folder(Vec<String>),
}

#[derive(Clone)]
pub struct Tree2 {
    root: Tree2Branch,
}

impl Tree2Branch {
    pub fn get_path(&self, elements: &Vec<&str>, depth: usize) -> Result<&Tree2Branch, String> {
        if depth == elements.len() {
            return Ok(self);
        }

        let element = elements[depth];

        if let Tree2Branch::Folder(_, hash_map) = self {
            if let Some(branch) = hash_map.get(element) {
                branch.get_path(elements, depth + 1)
            } else {
                Err("Invalid Path".into())
            }
        } else {
            return Err("This is a folder, not a file".into());
        }
    }

    pub fn to_repr(&self) -> Tree2Repr {
        match self {
            Tree2Branch::File(name) => Tree2Repr::File(name.clone()),
            Tree2Branch::Folder(_, hash_map) => {
                Tree2Repr::Folder(hash_map.keys().cloned().collect::<Vec<String>>())
            }
        }
    }
}

impl Tree2 {
    fn get(&self, path: &str) -> Result<Tree2Repr, String> {
        let elements = if path.is_empty() {
            Vec::new()
        } else {
            path.split("/").collect::<Vec<&str>>()
        };

        // let elements = path.split("/").collect::<Vec<&str>>();

        let branch = self.root.get_path(&elements, 0)?;
        Ok(branch.to_repr())
    }
}

impl Default for Tree2 {
    fn default() -> Self {
        Self {
            root: Tree2Branch::Folder(
                "ROOT".into(),
                HashMap::from([
                    ("File A".into(), Tree2Branch::File("A".into())),
                    ("File B".into(), Tree2Branch::File("B".into())),
                    (
                        "Folder C".into(),
                        Tree2Branch::Folder(
                            "A".into(),
                            HashMap::from([
                                ("File A".into(), Tree2Branch::File("A".into())),
                                ("File B".into(), Tree2Branch::File("B".into())),
                            ]),
                        ),
                    ),
                ]),
            ),
        }
    }
}

impl Server {
    pub async fn get_tree2(
        State(server): State<Server>,
        Path(path): Path<String>,
        Extension(_): Extension<CurrentUser>,
    ) -> Json<Value> {
        debug!("GET /api/interface/{}", path);

        let result = (|| -> Result<_, String> {
            let interface = server.tree.get(&path)?;

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
