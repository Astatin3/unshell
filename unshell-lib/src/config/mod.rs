use std::collections::HashMap;

// use bincode::{Decode, Encode};
// use serde::{Deserialize, Serialize};

use crate::{ModuleError, ModuleRuntime};

// /// Payload config that is instantiated
// #[derive(Serialize, Deserialize)]
// pub struct Config {
//     pub id: String,
//     pub key: String,
//     pub components: Vec<String>,
// }

pub struct PayloadConfig {
    pub id: &'static str,
    pub components: Vec<NamedComponent>,
    pub runtime_config: Vec<RuntimeConfig>,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub parent_component: &'static str,
    pub name: &'static str,
    pub config: HashMap<String, String>,
}

pub struct NamedComponent {
    pub name: &'static str,

    // + Sync + Sync + Sync + Sync + Sync + Sync + Sync + Sync
    pub get_interface: &'static (dyn Fn() -> Option<&'static (dyn InterfaceWrapper + Sync)> + Sync),
    pub start_runtime: &'static (
                 dyn Fn(&'static RuntimeConfig) -> Result<&'static dyn ModuleRuntime, ModuleError>
                     + Sync
             ),
}

/// Trait that wraps the get_interface<T>() function inside of components
pub trait InterfaceWrapper: Send + Sync {
    fn get_interface<T>() -> Option<T>
    where
        Self: Sized;
}
