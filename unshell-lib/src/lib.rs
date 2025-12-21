#![no_main]

pub mod config;
pub mod logger;

mod announcement;
use std::fmt::{self, Debug};

pub use announcement::Announcement;

pub type Result<T> = std::result::Result<T, ModuleError>;

///Generic error type for module-related operations.
#[derive(Debug)]
pub enum ModuleError {
    LibLoadingError(String),
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
