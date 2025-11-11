use chrono::{DateTime, Utc};

use crate::logger::{LogLevel, Logger, Record};

pub struct PrettyLogger;

// static TRACE_COLOR: &str = "\x1b[34m";
static DEBUG_COLOR: &str = "\x1b[36m";
static INFO_COLOR: &str = "\x1b[32m";
static WARN_COLOR: &str = "\x1b[33m";
static ERROR_COLOR: &str = "\x1b[31m";

static WHITE: &str = "\x1b[97m";
static OFF_WHITE: &str = "\x1b[37m";
static TIME_COLOR: &str = "\x1b[36m";
static GREY: &str = "\x1b[90m";

impl Logger for PrettyLogger {
    fn log(&self, message: Record) {
        let log_level = match message.log_level {
            LogLevel::Debug => format!("{DEBUG_COLOR}DBUG"),
            LogLevel::Info => format!("{INFO_COLOR}INFO"),
            LogLevel::Warn => format!("{WARN_COLOR}WARN"),
            LogLevel::Error => format!("{ERROR_COLOR}ERR!"),
        };

        let date: DateTime<Utc> = message.time.into();
        let date = date.to_rfc2822().to_string();

        let location = message.location.unwrap_or("".to_string());

        println!(
            "{OFF_WHITE}[{TIME_COLOR}{}{OFF_WHITE}] {} {WHITE}{} {GREY}{}{WHITE}",
            date, log_level, message.message, location
        );
    }
}

impl PrettyLogger {
    pub fn init() {
        crate::logger::set_logger_box(Box::new(PrettyLogger));
    }
}
