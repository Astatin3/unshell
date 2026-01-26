// use std::collections::HashMap;
use std::fmt::Debug;
use unshell::Result;
use unshell::config::RuntimeConfig;

use crate::ModuleRuntime;

pub struct PayloadConfig {
    pub id: &'static str,
    pub components: Vec<NamedComponent>,
    pub runtime_config: Vec<RuntimeConfig>,
}

#[derive(Clone)]
pub struct NamedComponent {
    pub name: &'static str,

    // + Sync + Sync + Sync + Sync + Sync + Sync + Sync + Sync
    pub get_interface: &'static (dyn Fn() -> Option<&'static (dyn InterfaceWrapper + Sync)> + Sync),
    pub start_runtime:
        &'static (dyn Fn(&'static RuntimeConfig) -> Result<Box<dyn ModuleRuntime>> + Sync),
}

impl Debug for NamedComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedComponent")
            .field("name", &self.name)
            // .field("get_interface", &self.get_interface)
            // .field("start_runtime", &self.start_runtime)
            .finish()
    }
}

/// Trait that wraps the get_interface<T>() function inside of components
pub trait InterfaceWrapper: Send + Sync {
    fn get_interface<T: 'static>(&self) -> Option<T>
    where
        Self: Sized;
}
