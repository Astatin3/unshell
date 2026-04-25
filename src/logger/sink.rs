use crate::logger::{LogLevel, Record};

/// Destination for log records.
///
/// Implement this trait to forward logs into a serial console, buffer, test
/// collector, or host integration.
pub trait Logger: Sync {
    /// Receives a single log record.
    fn log(&self, record: &Record<'_>);
}

/// Small filter-only logger for integrations that want a concrete type early.
///
/// This logger intentionally performs no output. It only exposes the same
/// filtering decision a real sink would make, which is useful while wiring up a
/// platform-specific backend later.
pub struct CompatibilityLogger {
    min_level: LogLevel,
}

impl CompatibilityLogger {
    /// Creates a logger that accepts `min_level` and anything more severe.
    #[must_use]
    pub const fn new(min_level: LogLevel) -> Self {
        Self { min_level }
    }

    /// Returns the minimum severity that passes the filter.
    #[must_use]
    pub const fn min_level(&self) -> LogLevel {
        self.min_level
    }

    /// Returns whether a record at `level` would be accepted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use unshell::logger::{CompatibilityLogger, LogLevel};
    ///
    /// let logger = CompatibilityLogger::new(LogLevel::Warn);
    ///
    /// assert!(!logger.accepts(LogLevel::Info));
    /// assert!(logger.accepts(LogLevel::Error));
    /// ```
    #[must_use]
    pub fn accepts(&self, level: LogLevel) -> bool {
        level >= self.min_level
    }
}

impl Logger for CompatibilityLogger {
    fn log(&self, record: &Record<'_>) {
        if !self.accepts(record.level) {
            return;
        }

        let _ = record;
    }
}

pub(crate) struct NullLogger;

impl Logger for NullLogger {
    fn log(&self, _record: &Record<'_>) {}
}
