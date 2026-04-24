use crate::logger::LogLevel;

/// A single log entry delivered to a [`crate::logger::Logger`].
///
/// The record borrows the formatted message from the logging call site so the
/// sink can inspect source context without owning additional state.
pub struct Record<'a> {
    /// Severity level for the entry.
    pub level: LogLevel,
    /// Human-readable message body.
    pub message: &'a str,
    /// Source file reported by `file!()` when available.
    pub file: Option<&'static str>,
    /// Source line reported by `line!()` when available.
    pub line: Option<u32>,
}

impl<'a> Record<'a> {
    /// Creates a new record from explicit parts.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use unshell::logger::{LogLevel, Record};
    ///
    /// let record = Record::new(LogLevel::Warn, "unexpected route", Some("router.rs"), Some(12));
    ///
    /// assert_eq!(record.level, LogLevel::Warn);
    /// assert_eq!(record.message, "unexpected route");
    /// ```
    #[must_use]
    pub const fn new(
        level: LogLevel,
        message: &'a str,
        file: Option<&'static str>,
        line: Option<u32>,
    ) -> Self {
        Self {
            level,
            message,
            file,
            line,
        }
    }
}
