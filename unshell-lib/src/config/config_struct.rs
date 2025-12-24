use serde_json::{Value, json};

use crate::{
    ModuleError, Result,
    config::{ConfigStructField, InterfaceData, InterfaceStruct, TreeMessage},
    warn,
};

pub type ConfigStructKeys = Vec<ConfigStructField>;
pub type ConfigStructValues = Vec<Value>;

pub struct Config {
    keys: ConfigStructKeys,
    values: ConfigStructValues,
}

impl Config {
    pub fn new(keys: ConfigStructKeys) -> Self {
        let values = keys
            .iter()
            .map(|key| match key {
                ConfigStructField::Header(_) => Value::Null,
                ConfigStructField::Text(_) => Value::Null,
                ConfigStructField::String { default, .. } => json!(default),
                ConfigStructField::Integer { default, .. } => json!(default),
            })
            .collect();

        Self { keys, values }
    }

    pub fn get(&mut self, message: TreeMessage) -> Result<TreeMessage> {
        match message {
            TreeMessage::State(InterfaceData::ConfigStruct(values)) => {
                self.values = values;
                Ok(TreeMessage::Success)
            }

            TreeMessage::RequestStruct => Ok(TreeMessage::Interface(
                InterfaceStruct::ConfigStruct(self.keys.clone()),
            )),
            TreeMessage::RequestState => Ok(TreeMessage::State(InterfaceData::ConfigStruct(
                self.values.clone(),
            ))),
            TreeMessage::RequestStructAndValue => Ok(TreeMessage::InterfaceAndValue(
                InterfaceStruct::ConfigStruct(self.keys.clone()),
                InterfaceData::ConfigStruct(self.values.clone()),
            )),

            _ => {
                warn!("Tree got invalid message");
                Err(ModuleError::Error("Invalid Request".into()))
            }
        }
    }
}
