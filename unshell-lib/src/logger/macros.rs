#[macro_export]
macro_rules! debug {
    ($fmt:tt) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Debug,
            String::from(unshell_obfuscate::file_symbol!()),
            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Debug,
            String::from(unshell_obfuscate::file_symbol!()),
            std::time::SystemTime::now(),
            log_result
        );
    }};
}

#[macro_export]
macro_rules! info {
    ($fmt:tt) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Info,
            String::from(unshell_obfuscate::file_symbol!()),
            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Info,
            String::from(unshell_obfuscate::file_symbol!()),
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
            String::from(unshell_obfuscate::file_symbol!()),
            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Warn,
            String::from(unshell_obfuscate::file_symbol!()),
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
            String::from(unshell_obfuscate::file_symbol!()),
            std::time::SystemTime::now(),
            log_result
        );
    }};
    ($fmt:tt, $($arg:expr),*) => {{
        let log_result = unshell_obfuscate::format_obs!($fmt, $($arg),*);

        $crate::logger::add_record(
            $crate::logger::LogLevel::Error,
            String::from(unshell_obfuscate::file_symbol!()),
            std::time::SystemTime::now(),
            log_result
        );
    }};
}
