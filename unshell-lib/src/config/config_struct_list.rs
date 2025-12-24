use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ModuleError, Result,
    config::{ConfigStructField, InterfaceData, InterfaceStruct, Tree, TreeMessage},
    warn,
};

pub type ConfigStructListKeys = Vec<ConfigStructField>;
pub type ConfigStructListValues = Vec<Vec<Value>>;

pub struct ConfigStructList {
    keys: ConfigStructListKeys,
    values: ConfigStructListValues,
}

impl ConfigStructList {
    pub fn new(keys: ConfigStructListKeys) -> Self {
        // let values = keys
        //     .iter()
        //     .map(|key| match key {
        //         ConfigStructField::Header(_) => Value::Null,
        //         ConfigStructField::Text(_) => Value::Null,
        //         ConfigStructField::String { default, .. } => json!(default),
        //         ConfigStructField::Integer { default, .. } => json!(default),
        //     })
        //     .collect();

        Self {
            keys,
            values: Vec::new(),
        }
    }
}

// impl Tree for ConfigStructList {}
