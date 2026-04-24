use crate::logger::{CompatibilityLogger, LogLevel};

#[test]
fn level_labels_match_expected_output() {
    assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
    assert_eq!(LogLevel::Info.as_str(), "INFO");
    assert_eq!(LogLevel::Warn.as_str(), "WARN");
    assert_eq!(LogLevel::Error.as_str(), "ERROR");
}

#[test]
fn compatibility_logger_filters_lower_levels() {
    let logger = CompatibilityLogger::new(LogLevel::Warn);

    assert!(!logger.accepts(LogLevel::Debug));
    assert!(!logger.accepts(LogLevel::Info));
    assert!(logger.accepts(LogLevel::Warn));
    assert!(logger.accepts(LogLevel::Error));
}
