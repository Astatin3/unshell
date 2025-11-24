// Macros that are used that just drop the inside variables

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
