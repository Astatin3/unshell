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
