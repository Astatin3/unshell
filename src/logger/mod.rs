//! Logging infrastructure for unshell.
//!
//! This module provides a pluggable logging system with support for
//! compile-time feature flags to enable or disable logging.
//!
//! # Features
//!
//! - **Feature-gated**: Logging can be disabled at compile time for smaller binaries
//! - **Custom loggers**: Implement the `Logger` trait for custom output
//! - **Structured records**: Log records include level, location, time, and message
//! - **FFI support**: C-compatible setup function for external initialization
//!
//! # Usage
//!
//! ```rust
//! use unshell::logger::{LogLevel, Record, Logger};
//!
//! // Implement custom logger
//! struct MyLogger;
//!
//! impl Logger for MyLogger {
//!     fn log(&self, record: Record) {
//!         println!("[{:?}] {}", record.log_level, record.message);
//!     }
//! }
//!
//! // Set as global logger
//! unshell::logger::set_logger(&MyLogger);
//! ```
//!
//! # Log Levels
//!
//! ```rust
//! use unshell::logger::LogLevel;
//!
//! let level = LogLevel::Debug;
//! let level = LogLevel::Info;
//! let level = LogLevel::Warn;
//! let level = LogLevel::Error;
//! ```
//!
//! # Feature Flags
//!
//! - `log`: Enable logging functionality
//! - `log_debug`: Enable debug-level logging
//!
//! When the `log` feature is disabled, the macros module is replaced with
//! no-op implementations to reduce binary size.

// Choose if the macros are enabled based on the feature setting
#[cfg(feature = "log")]
pub mod macros;

#[cfg(not(feature = "log"))]
pub mod macros_disabled;

mod pretty_logger;

use std::time::SystemTime;

pub use pretty_logger::log;
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
