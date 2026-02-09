#[macro_export]
macro_rules! log {
    ($level:expr, $fmt:tt) => {{
        use $crate::obfuscate;
        let log_result = obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $level,

            #[cfg(feature = "log_debug")]
            Some(String::from(obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($level:expr, $fmt:tt, $($arg:expr),*) => {{
        use $crate::obfuscate;
        let log_result = obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $level,

            #[cfg(feature = "log_debug")]
            Some(String::from(obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Debug, $($arg)*)
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Info, $($arg)*)
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Warn, $($arg)*)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log!($crate::logger::LogLevel::Error, $($arg)*)
    };
}
