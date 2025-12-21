mod manager;
mod module;
mod module_interface;
pub mod network;
mod proc_load;

pub mod interface;

use std::sync::{Arc, Mutex};

pub use manager::Manager;
pub use module::Module;

pub use interface::{InterfaceWrapper, NamedComponent};

extern crate unshell_lib;
use unshell_lib::Result;

/// Trait for defining modules that have a runtime.
pub trait ModuleRuntime: Send + Sync {
    fn init(&mut self, manager: Arc<Mutex<Manager>>) -> Result<()>;

    /// Returns true if the module is running.
    /// After returning false, the module will be dropped.
    fn is_running(&self) -> bool;
    /// Consumes the module, implementation should kill whatever is running.
    fn kill(self: Box<Self>);
}
