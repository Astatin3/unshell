//! Logging macros for unshell.
//!
//! Provides convenient logging macros that integrate with the
//! feature-gated logging system.
//!
//! # Usage
//!
//! ```rust
//! use unshell::{info, warn, error, debug};
//!
//! info!("Application started");
//! warn!("Configuration file not found, using defaults");
//! error!("Failed to connect: {}", err);
//! debug!("Processing item {}", idx);
//! ```
//!
//! # Feature Flags
//!
//! - `log`: Enable logging (required for any output)
//! - `log_debug`: Include file location in log records

#[macro_export]
macro_rules! log {
    ($level:expr, $fmt:tt) => {{
        use $crate::obfuscate::format_sym;
        let log_result = format_sym!($fmt);

        $crate::logger::add_record(
            $level,

            #[cfg(feature = "log_debug")]
            Some(String::from($crate::obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($level:expr, $fmt:tt, $($arg:expr),*) => {{
        use $crate::obfuscate::format_sym;
        let log_result = format_sym!($fmt, $($arg),*);

        $crate::logger::add_record(
            $level,

            #[cfg(feature = "log_debug")]
            Some(String::from($crate::obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
}

/// Log a debug-level message.
///
/// Only produces output when the `log` feature is enabled.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Debug, $($arg)*)
    };
}

/// Log an info-level message.
///
/// Only produces output when the `log` feature is enabled.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Info, $($arg)*)
    };
}

/// Log a warning-level message.
///
/// Only produces output when the `log` feature is enabled.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Warn, $($arg)*)
    };
}

/// Log an error-level message.
///
/// Only produces output when the `log` feature is enabled.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Error, $($arg)*)
    };
}
