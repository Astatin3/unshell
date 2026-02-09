use serde_json::Value;

use crate::Server;

impl Server {
    pub fn get_blobs(&self) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }
}
