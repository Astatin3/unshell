pub mod macros;
mod pretty_logger;

use std::time::SystemTime;

pub use pretty_logger::PrettyLogger;

static mut LOGGER: &dyn Logger = &DefaultLogger;

#[derive(Debug)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug)]
pub struct Record {
    log_level: LogLevel,
    location: String,
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

pub fn set_logger_box(logger: Box<dyn Logger>) {
    unsafe {
        LOGGER = Box::leak(logger);
    }
}

pub fn set_logger(logger: &'static dyn Logger) {
    unsafe {
        LOGGER = logger;
    }
}

pub fn add_record(log_level: LogLevel, location: String, time: SystemTime, message: String) {
    logger().log(Record {
        log_level,
        location,
        time,
        message,
    });
}

pub fn logger<'a>() -> &'static dyn Logger {
    unsafe { LOGGER }
}

#[allow(dead_code, improper_ctypes_definitions)]
pub type SetupLogger = extern "C" fn(logger: &'static dyn Logger);

#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn setup_logger(logger: &'static dyn Logger) {
    set_logger(logger);
}
