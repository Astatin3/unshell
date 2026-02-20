//! No-op logging macros when logging is disabled.
//!
//! When the `log` feature is not enabled, these macros are used instead.
//! They compile to no code, ensuring zero overhead when logging is disabled.
//!
//! # Usage
//!
//! These macros are automatically used when the `log` feature is disabled.
//! No code changes required - just don't enable the feature.

// Macros that are used that just drop the inside variables

/// No-op debug logging macro.
///
/// Expands to nothing when `log` feature is disabled.
#[macro_export]
macro_rules! debug {
    ($fmt:tt) => {{
        let _ = $fmt;
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let _ = $fmt;
        $(let _ = $arg;)*
    }};
}

/// No-op info logging macro.
///
/// Expands to nothing when `log` feature is disabled.
#[macro_export]
macro_rules! info {
    ($fmt:tt) => {{
        let _ = $fmt;
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let _ = $fmt;
        $(let _ = $arg;)*
    }};
}

/// No-op warn logging macro.
///
/// Expands to nothing when `log` feature is disabled.
#[macro_export]
macro_rules! warn {
    ($fmt:tt) => {{
        let _ = $fmt;
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let _ = $fmt;
        $(let _ = $arg;)*
    }};
}

/// No-op error logging macro.
///
/// Expands to nothing when `log` feature is disabled.
#[macro_export]
macro_rules! error {
    ($fmt:tt) => {{
        let _ = $fmt;
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let _ = $fmt;
        $(let _ = $arg;)*
    }};
}
