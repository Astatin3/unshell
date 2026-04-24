/// Logs a message at [`crate::logger::LogLevel::Debug`].
///
/// # Examples
///
/// ```rust
/// use unshell::debug;
///
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

/// Logs a message at [`crate::logger::LogLevel::Info`].
///
/// # Examples
///
/// ```rust
/// use unshell::info;
///
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

/// Logs a message at [`crate::logger::LogLevel::Warn`].
///
/// # Examples
///
/// ```rust
/// use unshell::warn;
///
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

/// Logs a message at [`crate::logger::LogLevel::Error`].
///
/// # Examples
///
/// ```rust
/// use unshell::error;
///
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
