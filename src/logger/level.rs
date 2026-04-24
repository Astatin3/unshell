/// Severity level carried by a [`crate::logger::Record`].
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
    /// Returns a short uppercase label suitable for log prefixes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use unshell::logger::LogLevel;
    ///
    /// assert_eq!(LogLevel::Info.as_str(), "INFO");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}
