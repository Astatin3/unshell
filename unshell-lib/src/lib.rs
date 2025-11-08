#![no_main]
#[macro_use]
extern crate log;

pub mod client;
pub mod module;
pub mod server;

mod announcement;
use std::sync::{Arc, Mutex};

pub use announcement::Announcement;

use crate::module::{Interface, Manager};

///Generic error type for module-related operations.
#[derive(Debug)]
pub enum ModuleError {
    LibLoadingError(libloading::Error),
    LogError(log::SetLoggerError),
    LinkError(String),
    Error(String),
}

/// Trait for defining modules that have a runtime.
pub trait ModuleRuntime: Send {
    /// Returns true if the module is running.
    /// After returning false, the module will be dropped.
    fn is_running(&self) -> bool;
    /// Consumes the module, implementation should kill whatever is running.
    fn kill(self: Box<Self>);
}

pub trait Component {
    fn name(&self) -> &'static str;
    fn start_runtime(&self, manager: Arc<Mutex<Manager>>) -> Option<Box<dyn ModuleRuntime>>;
    fn get_interface(&self) -> Box<dyn Interface>;
    fn clone_box(&self) -> Box<dyn Component>;
}

impl Clone for Box<dyn Component> {
    fn clone(&self) -> Box<dyn Component> {
        self.clone_box()
    }
}
