#![no_main]

pub mod config;
mod error;
pub mod logger;

mod announcement;

pub use error::{ModuleError, Result};

pub use announcement::Announcement;

// Re-exports
// pub use unshell_crypt;
pub use unshell_obfuscate;

// pub trait Component {
//     fn name(&self) -> &'static str;
//     // fn start_runtime(&self, manager: Arc<Mutex<Manager>>) -> Option<Box<dyn ModuleRuntime>>;

//     fn get_interface(&self) -> Box<dyn Interface>;
//     fn clone_box(&self) -> Box<dyn Component>;
// }

// impl Clone for Box<dyn Component> {
//     fn clone(&self) -> Box<dyn Component> {
//         self.clone_box()
//     }
// }
