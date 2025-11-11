#[cfg(feature = "log_debug")]
#[macro_export]
macro_rules! debug {
    ($fmt:tt) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Debug,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Debug,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
}

#[cfg(not(feature = "log_debug"))]
#[macro_export]
macro_rules! debug {
    ($fmt:tt) => {{
        let _ = $fmt;
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let _ = $fmt;
        let _ = ($($arg),*);
    }};
}

#[macro_export]
macro_rules! info {
    ($fmt:tt) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Info,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Info,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
}

#[macro_export]
macro_rules! warn {
    ($fmt:tt) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Warn,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Warn,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($fmt:tt) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Error,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Error,

            #[cfg(feature = "log_debug")]
            Some(String::from(unshell_obfuscate::file_symbol!())),
            #[cfg(not(feature = "log_debug"))]
            None,

            std::time::SystemTime::now(),
            log_result
        );
    }};
}
