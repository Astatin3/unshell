use core::cell::UnsafeCell;

use super::sink::NullLogger;
use crate::logger::{LogLevel, Logger, Record};

struct LoggerCell(UnsafeCell<&'static dyn Logger>);

impl LoggerCell {
    const fn new(logger: &'static dyn Logger) -> Self {
        Self(UnsafeCell::new(logger))
    }

    fn set(&self, logger: &'static dyn Logger) {
        // Rationale: the logger is installed during single-threaded startup.
        // Keeping the unsafety inside this tiny cell is easier to audit than
        // exposing `static mut` references throughout the module.
        unsafe {
            *self.0.get() = logger;
        }
    }

    fn get(&self) -> &'static dyn Logger {
        // Rationale: after startup the stored reference is treated as immutable,
        // so reading the copied trait object reference is safe under the module
        // contract documented on `set_logger`.
        unsafe { *self.0.get() }
    }
}

// SAFETY: access is funneled through the startup-only `set` contract and
// read-only `get` path above. `Logger: Sync` ensures sharing the sink is valid.
unsafe impl Sync for LoggerCell {}

static GLOBAL_LOGGER: LoggerCell = LoggerCell::new(&NullLogger);

/// Installs the global logger used by the logging macros.
///
/// Call this once during startup, before any concurrent execution begins.
/// Replacing the logger later would require external synchronization and is not
/// supported by this module's contract.
///
/// # Examples
///
/// ```rust,no_run
/// use unshell::logger::{Logger, Record, set_logger};
///
/// struct MyLogger;
///
/// impl Logger for MyLogger {
///     fn log(&self, _record: &Record<'_>) {}
/// }
///
/// static LOGGER: MyLogger = MyLogger;
/// set_logger(&LOGGER);
/// ```
pub fn set_logger(logger: &'static dyn Logger) {
    GLOBAL_LOGGER.set(logger);
}

/// Returns the currently installed global logger.
#[must_use]
pub fn global_logger() -> &'static dyn Logger {
    GLOBAL_LOGGER.get()
}

/// Sends a single record through the installed global logger.
///
/// Most code should prefer the exported logging macros.
pub fn log(level: LogLevel, message: &str, file: Option<&'static str>, line: Option<u32>) {
    global_logger().log(&Record::new(level, message, file, line));
}
