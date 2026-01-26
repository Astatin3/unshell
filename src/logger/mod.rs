// Choose if the macros are enabled based on the feature setting
#[cfg(feature = "log")]
pub mod macros;

#[cfg(not(feature = "log"))]
pub mod macros_disabled;

mod pretty_logger;

use std::time::SystemTime;

pub use pretty_logger::PrettyLogger;

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
    time: SystemTime,
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
    }
}

pub fn set_logger(logger: &'static dyn Logger) {
    unsafe {
        LOGGER = logger;
    }
}

pub fn add_record(
    log_level: LogLevel,
    location: Option<String>,
    time: SystemTime,
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
