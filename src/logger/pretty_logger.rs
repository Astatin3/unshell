//! Pretty console logger implementation.
//!
//! Provides a colored, formatted logger for console output.
//! Supports custom output handlers and ANSI color codes.
//!
//! # Usage
//!
//! ```rust
//! use unshell::logger::PrettyLogger;
//!
//! // Initialize with console output
//! PrettyLogger::init();
//!
//! // Or with custom output handler
//! PrettyLogger::init_output(|record| {
//!     // Custom handling
//! });
//! ```

use chrono::{DateTime, Utc};

use crate::logger::{LogLevel, Logger, Record};

/// A logger that outputs formatted, colored messages to console.
///
/// Supports ANSI color codes and optional custom output handlers.
/// Output format: `[timestamp] LEVEL message [location]`
pub struct PrettyLogger {
    output: Option<Box<dyn Fn(&Record)>>,
}

// static TRACE_COLOR: &str = "\x1b[34m";
static DEBUG_COLOR: &str = "\x1b[36m";
static INFO_COLOR: &str = "\x1b[32m";
static WARN_COLOR: &str = "\x1b[33m";
static ERROR_COLOR: &str = "\x1b[31m";

static WHITE: &str = "\x1b[97m";
static OFF_WHITE: &str = "\x1b[37m";
static TIME_COLOR: &str = "\x1b[36m";
static GREY: &str = "\x1b[90m";

impl Logger for PrettyLogger {
    fn log(&self, message: Record) {
        if let Some(ref func) = self.output {
            (*func)(&message)
        }

        log(&message);
    }
}

/// Format and print a log record to console.
///
/// Uses ANSI color codes for level differentiation:
/// - Debug: Cyan
/// - Info: Green  
/// - Warn: Yellow
/// - Error: Red
pub fn log(message: &Record) {
    let log_level = match message.log_level {
        LogLevel::Debug => format!("{DEBUG_COLOR}DBUG"),
        LogLevel::Info => format!("{INFO_COLOR}INFO"),
        LogLevel::Warn => format!("{WARN_COLOR}WARN"),
        LogLevel::Error => format!("{ERROR_COLOR}ERR!"),
    };

    let date: DateTime<Utc> = message.time.into();
    let date = date.to_rfc2822().to_string();

    let location = if let Some(ref location) = message.location {
        location
    } else {
        &String::new()
    };

    println!(
        "{OFF_WHITE}[{TIME_COLOR}{}{OFF_WHITE}] {} {WHITE}{} {GREY}{}{WHITE}",
        date, log_level, message.message, location
    );
}

impl PrettyLogger {
    /// Initialize with default console output.
    pub fn init() {
        crate::logger::set_logger_box(Box::new(PrettyLogger { output: None }));
    }

    /// Initialize with custom output handler.
    ///
    /// The handler receives each `Record` for custom processing.
    pub fn init_output<T>(output: T)
    where
        T: Fn(&Record) + 'static,
    {
        crate::logger::set_logger_box(Box::new(PrettyLogger {
            output: Some(Box::new(output)),
        }));
    }
}
