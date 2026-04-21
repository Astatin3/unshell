//! # Logger Module
//!
//! A lightweight, no_std-compatible logging system.
//!
//! ## Usage
//!
//! ```rust
//! use unshell::{info, warn, error};
//! use unshell::logger::Logger;
//!
//! // Uses the default (no-op) logger until one is installed.
//! info!("Starting up");
//! warn!("Something is off");
//! error!("Critical failure");
//! ```
//!
//! ## Installing a logger
//!
//! Call [`set_logger`] with any type that implements [`Logger`]:
//!
//! ```rust,no_run
//! use unshell::logger::{Logger, LogLevel, Record, set_logger};
//!
//! struct StdoutLogger;
//! impl Logger for StdoutLogger {
//!     fn log(&self, record: &Record<'_>) {
//!         // In a no_std environment you would use the `unix-print` crate
//!         // or write to a pre-opened file descriptor.
//!         let _ = record; // placeholder
//!     }
//! }
//!
//! static MY_LOGGER: StdoutLogger = StdoutLogger;
//! set_logger(&MY_LOGGER);
//! ```
//!
//! ## Thread safety
//!
//! The global logger pointer is set **once at startup**, before any threads
//! are spawned. After that, it is only read (never written). This is safe
//! because:
//!
//! 1. The payload is single-threaded.
//! 2. The router and CLI set the logger before spawning node threads.
//!
//! If you need to change the logger after threads start, synchronise access
//! with a `Mutex` or an atomic pointer in your logger implementation.

// ---------------------------------------------------------------------------
// Log levels
// ---------------------------------------------------------------------------

/// The severity level of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Verbose diagnostic information.
    Debug,
    /// Normal operational messages.
    Info,
    /// Something unexpected happened but execution can continue.
    Warn,
    /// A serious error occurred.
    Error,
}

impl LogLevel {
    /// Short uppercase label, suitable for log line prefixes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use unshell::logger::LogLevel;
    /// assert_eq!(LogLevel::Info.as_str(), "INFO");
    /// ```
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

// ---------------------------------------------------------------------------
// Log record
// ---------------------------------------------------------------------------

/// A single log entry passed to a [`Logger`].
///
/// Borrows from the call site to avoid heap allocation on the hot path.
pub struct Record<'a> {
    /// Severity level.
    pub level: LogLevel,
    /// The log message.
    pub message: &'a str,
    /// Source file, if available (e.g. `file!()`).
    pub file: Option<&'static str>,
    /// Source line number, if available (e.g. `line!()`).
    pub line: Option<u32>,
}

// ---------------------------------------------------------------------------
// Logger trait
// ---------------------------------------------------------------------------

/// A sink for log records.
///
/// Implement this to direct log output wherever you want (stdout, a file,
/// a TCP connection, a memory buffer for tests).
pub trait Logger: Sync {
    /// Receive and process a log record.
    fn log(&self, record: &Record<'_>);
}

// ---------------------------------------------------------------------------
// Global logger state
// ---------------------------------------------------------------------------

/// The no-op logger used before any logger is installed.
struct NullLogger;
impl Logger for NullLogger {
    fn log(&self, _record: &Record<'_>) {}
}

/// The global logger pointer.
///
/// Written once at startup via [`set_logger`], then only read.
/// # Safety
/// This is `static mut` to avoid a dependency on synchronisation primitives
/// in a no_std context. It is safe as long as `set_logger` is called before
/// any threads are spawned (see module-level docs).
static mut GLOBAL_LOGGER: &dyn Logger = &NullLogger;

/// Install a new global logger.
///
/// Must be called **before** spawning any threads. After this call, all
/// `info!`, `warn!`, `error!`, and `debug!` macros route to this logger.
///
/// # Safety
///
/// This function writes to a `static mut`. It is safe when called exactly
/// once at program startup before any other threads exist.
///
/// # Example
///
/// ```rust,no_run
/// use unshell::logger::{Logger, Record, set_logger};
///
/// static MY_LOGGER: MyLogger = MyLogger;
/// set_logger(&MY_LOGGER);
///
/// # struct MyLogger;
/// # impl Logger for MyLogger { fn log(&self, _: &Record<'_>) {} }
/// ```
pub fn set_logger(logger: &'static dyn Logger) {
    // SAFETY: called once at startup before any threads are spawned.
    #[allow(static_mut_refs)]
    unsafe {
        GLOBAL_LOGGER = logger;
    }
}

/// Return a reference to the currently installed logger.
///
/// Used internally by the logging macros.
#[must_use]
pub fn global_logger() -> &'static dyn Logger {
    // SAFETY: GLOBAL_LOGGER is only written once (at startup) and is
    // read-only thereafter. No data race is possible.
    #[allow(static_mut_refs)]
    unsafe {
        GLOBAL_LOGGER
    }
}

/// Log a record through the global logger.
///
/// This is the low-level function called by the macros. Prefer using the
/// `info!`, `warn!`, `error!`, and `debug!` macros directly.
pub fn log(level: LogLevel, message: &str, file: Option<&'static str>, line: Option<u32>) {
    global_logger().log(&Record {
        level,
        message,
        file,
        line,
    });
}

// ---------------------------------------------------------------------------
// A minimal stdout logger for use in std binaries (router, CLI)
// ---------------------------------------------------------------------------

/// A simple logger that prints to stderr.
///
/// Suitable for the router and operator CLI binaries.
/// Do not use in the payload binary (which may not have stderr available).
///
/// # Example
///
/// ```rust,no_run
/// use unshell::logger::{StderrLogger, set_logger};
///
/// static LOGGER: StderrLogger = StderrLogger::new(unshell::logger::LogLevel::Info);
/// set_logger(&LOGGER);
/// ```
pub struct StderrLogger {
    /// Minimum level to log. Records below this level are discarded.
    min_level: LogLevel,
}

impl StderrLogger {
    /// Create a new `StderrLogger` that logs records at `min_level` and above.
    ///
    /// # Example
    ///
    /// ```rust
    /// use unshell::logger::{StderrLogger, LogLevel};
    /// let logger = StderrLogger::new(LogLevel::Info);
    /// ```
    #[must_use]
    pub const fn new(min_level: LogLevel) -> Self {
        Self { min_level }
    }
}

impl Logger for StderrLogger {
    fn log(&self, record: &Record<'_>) {
        if record.level < self.min_level {
            return;
        }
        // eprintln! and String require std (available only with the `tcp` feature).
        // In no_std builds this method is a no-op. The payload uses a different
        // logger (or the null logger) in no_std contexts.
        #[cfg(feature = "tcp")]
        {
            use alloc::string::String;
            let location = match (record.file, record.line) {
                (Some(f), Some(l)) => {
                    let mut s = String::from(f);
                    s.push(':');
                    s.push_str(&format!("{l}"));
                    s
                }
                _ => String::new(),
            };
            if location.is_empty() {
                eprintln!("[{}] {}", record.level.as_str(), record.message);
            } else {
                eprintln!("[{}] {} - {}", record.level.as_str(), record.message, location);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Logging macros
// ---------------------------------------------------------------------------

/// Log at [`LogLevel::Debug`] level.
///
/// ```rust
/// use unshell::debug;
/// debug!("loop iteration {}", 42);
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Debug,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        )
    };
}

/// Log at [`LogLevel::Info`] level.
///
/// ```rust
/// use unshell::info;
/// info!("server started on port {}", 9000);
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Info,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        )
    };
}

/// Log at [`LogLevel::Warn`] level.
///
/// ```rust
/// use unshell::warn;
/// warn!("unexpected path: {}", "/unknown");
/// ```
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Warn,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        )
    };
}

/// Log at [`LogLevel::Error`] level.
///
/// ```rust
/// use unshell::error;
/// error!("connection failed: {}", "timeout");
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Error,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        )
    };
}
