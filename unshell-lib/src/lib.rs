#![no_main]

pub mod client;
pub mod config;
pub mod logger;
pub mod module;
pub mod server;

mod components;
pub use components::get_components;

mod announcement;
use std::{
    fmt,
    sync::{Arc, Mutex},
};

pub use announcement::Announcement;

use crate::module::{Interface, Manager};

///Generic error type for module-related operations.
#[derive(Debug)]
pub enum ModuleError {
    LibLoadingError(libloading::Error),
    // LogError(log::SetLoggerError),
    LinkError(String),
    CryptError(String),
    Error(String),
}

impl std::error::Error for ModuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        Some(self)
    }
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(format!("{:?}", self).as_str())
    }
}

/// Trait for defining modules that have a runtime.
pub trait ModuleRuntime: Send + Sync {
    /// Returns true if the module is running.
    /// After returning false, the module will be dropped.
    fn is_running(&self) -> bool;
    /// Consumes the module, implementation should kill whatever is running.
    fn kill(self: Box<Self>);
}

pub trait Component {
    fn name(&self) -> &'static str;
    // fn start_runtime(&self, manager: Arc<Mutex<Manager>>) -> Option<Box<dyn ModuleRuntime>>;

    fn get_interface(&self) -> Box<dyn Interface>;
    fn clone_box(&self) -> Box<dyn Component>;
}

impl Clone for Box<dyn Component> {
    fn clone(&self) -> Box<dyn Component> {
        self.clone_box()
    }
}
