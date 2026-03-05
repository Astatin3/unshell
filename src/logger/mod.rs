// Choose if the macros are enabled based on the feature setting
#[cfg(feature = "log")]
mod log_enabled;

#[cfg(not(feature = "log"))]
mod log_disabled;

mod pretty_logger;

use std::time::SystemTime;

pub use pretty_logger::PrettyLogger;
pub use pretty_logger::log;

pub static mut IS_DEFAULT_LOGGER: bool = true;
static mut LOGGER: &dyn Logger = &DefaultLogger;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Record {
    log_level: LogLevel,
    location: Option<String>,
    // line: u32,
    time: Option<SystemTime>,
    message: String,
}

pub trait Logger {
    fn log(&self, log: Record);
}

struct DefaultLogger;

impl Logger for DefaultLogger {
    fn log(&self, _: Record) {}
}

#[allow(unused_variables)]
pub fn set_logger_box(logger: Box<dyn Logger>) {
    #[cfg(feature = "log")]
    unsafe {
        LOGGER = Box::leak(logger);
        IS_DEFAULT_LOGGER = false;
    }
}

pub fn set_logger(logger: &'static dyn Logger) {
    unsafe {
        LOGGER = logger;
        IS_DEFAULT_LOGGER = false;
    }
}

pub fn add_record(
    log_level: LogLevel,
    location: Option<String>,
    time: Option<SystemTime>,
    message: String,
) {
    logger().log(Record {
        log_level,
        location,
        time,
        message,
    });
}

pub fn logger() -> &'static dyn Logger {
    unsafe { LOGGER }
}

#[allow(dead_code, improper_ctypes_definitions)]
pub type SetupLogger = extern "C" fn(logger: &'static dyn Logger);

#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn setup_logger(logger: &'static dyn Logger) {
    set_logger(logger);
}

// Macro Definitions
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Debug, $($arg)*)
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Info, $($arg)*)
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Warn, $($arg)*)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Error, $($arg)*)
    };
}
