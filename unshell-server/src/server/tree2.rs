use axum::{
    Extension, Json,
    extract::{Path, State},
};

use serde_json::Value;
use unshell_lib::{
    ModuleError,
    config::{Tree, TreeMessage},
    debug,
};

use crate::{Server, auth::structs::CurrentUser};

impl Server {
    pub async fn get_tree2_root(
        State(server): State<Server>,
        // Extension(extension): Extension<CurrentUser>,
    ) -> Json<Value> {
        Self::get_tree2(State(server), Path("".into())).await
    }

    pub async fn get_tree2(
        State(mut server): State<Server>,
        Path(path): Path<String>,
        // Extension(_): Extension<CurrentUser>,
    ) -> Json<Value> {
        debug!("GET /api/interface/{}", path);

        let result = server
            .get(&path, TreeMessage::RequestStructAndValue)
            .map_err(|e| ModuleError::CryptError(e.to_string()));

        Json(serde_json::to_value(result).unwrap())
    }

    pub async fn post_tree2(
        State(mut server): State<Server>,
        Path(path): Path<String>,
        // Extension(_): Extension<CurrentUser>,
        Json(tree_message): Json<TreeMessage>,
    ) -> Json<Value> {
        debug!("POST /api/interface/");

        // Json(Value::Null)

        let result = server
            .get(&path, tree_message)
            .map_err(|e| ModuleError::CryptError(e.to_string()));

        Json(serde_json::to_value(result).unwrap())
    }
}
