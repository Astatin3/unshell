use crate::logger::{LogLevel, Logger, Record};

pub struct PrettyLogger {
    output: Option<Box<dyn Fn(&Record)>>,
}

impl Logger for PrettyLogger {
    fn log(&self, message: Record) {
        if let Some(ref func) = self.output {
            (*func)(&message)
        }

        log(&message);
    }
}

pub fn log(message: &Record) {
    static DEBUG_COLOR: &str = "\x1b[36m";
    static INFO_COLOR: &str = "\x1b[32m";
    static WARN_COLOR: &str = "\x1b[33m";
    static ERROR_COLOR: &str = "\x1b[31m";

    let log_level = match message.log_level {
        LogLevel::Debug => format!("{DEBUG_COLOR}DBUG"),
        LogLevel::Info => format!("{INFO_COLOR}INFO"),
        LogLevel::Warn => format!("{WARN_COLOR}WARN"),
        LogLevel::Error => format!("{ERROR_COLOR}ERR!"),
    };

    match (message.time, &message.location) {
        (None, None) => {
            static WHITE: &str = "\x1b[97m";

            println!("{} {WHITE}{}", log_level, message.message,);
        }

        #[cfg(feature = "log_debug")]
        (Some(time), Some(location)) => {
            use chrono::{DateTime, Utc};

            let date: DateTime<Utc> = time.into();

            static WHITE: &str = "\x1b[97m";
            static OFF_WHITE: &str = "\x1b[37m";
            static TIME_COLOR: &str = "\x1b[36m";
            static GREY: &str = "\x1b[90m";

            println!(
                "{OFF_WHITE}[{TIME_COLOR}{}{OFF_WHITE}] {} {WHITE}{} {GREY}{}{WHITE}",
                date, log_level, message.message, location
            );
        }

        _ => unreachable!("Invalid log configuration"),
    }
}

impl PrettyLogger {
    pub fn init() {
        if unsafe { crate::logger::IS_DEFAULT_LOGGER } {
            crate::logger::set_logger_box(Box::new(PrettyLogger { output: None }));
        }
    }

    pub fn init_output<T>(output: T)
    where
        T: Fn(&Record) + 'static,
    {
        if !unsafe { crate::logger::IS_DEFAULT_LOGGER } {
            crate::logger::set_logger_box(Box::new(PrettyLogger {
                output: Some(Box::new(output)),
            }));
        }
    }
}
