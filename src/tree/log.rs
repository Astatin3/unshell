//! Tree-integrated logging system.
//!
//! Provides logging that pipes through the tree structure,
//! allowing remote access to logs via tree messages.
//!
//! # Usage
//!
//! ```rust
//! use unshell::tree::{Branch, TreeElement};
//!
//! let mut branch = Branch::new("root");
//! branch.init_logger();
//!
//! // Now logs can be retrieved via tree messages
//! // branch.send_message(json!("logger"), json!("Get"));
//! ```

/// Implement logging for the manager
use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};

use crate::{
    logger::{Logger, Record},
    tree::{symbols, Branch, TreeElement},
};

/// Logger transmitter - sends logs to channel
struct LoggerTX(Sender<Record>);

/// Logger receiver - exposes logs as TreeElement
struct LoggerRX(Receiver<Record>);

impl Branch {
    /// Initiate the unshell logger for the local binary, piped through the manager.
    ///
    /// This allows access to logs through the tree structure.
    /// Logs can be retrieved using `Get` command or queue length with `GetLength`.
    pub fn init_logger(&mut self) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (tx, rx) = (LoggerTX(tx), LoggerRX(rx));

        crate::logger::set_logger_box(Box::new(tx));
        self.add_child(symbols::LOGGER.to_string(), Box::new(rx));
    }
}

impl Logger for LoggerTX {
    fn log(&self, log: crate::logger::Record) {
        // This will never panic if the program is operating properly
        self.0.send(log).unwrap();
    }
}

impl TreeElement for LoggerRX {
    fn get_type(&self) -> Value {
        json!(symbols::TYPE_QUEUE)
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match (target, message) {
            (Value::Null, Value::String(message)) => match message.as_ref() {
                symbols::CMD_GET => {
                    if !self.0.is_empty() {
                        json!(self.0.recv().unwrap())
                    } else {
                        json!(Value::Null)
                    }
                }
                symbols::CMD_GET_LENGTH => {
                    json!(self.0.len())
                }
                _ => json!(symbols::ERR_INVALID_COMMAND),
            },
            _ => json!(symbols::ERR_INVALID_CHILD),
        }
    }
}
