/// Implement logging for the manager
use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};

use crate::{
    logger::{Logger, Record},
    tree::{symbols, Tree, TreeElement},
};

struct LoggerTX(Sender<Record>);
struct LoggerRX(Receiver<Record>);

impl Tree {
    /// Initiate the unshell logger for the local binary, piped through the manager
    /// This will allow access to the logs through the tree
    pub fn init_logger(&mut self) {
        // Create the logger through the TX element of the manager

        let (tx, rx) = crossbeam_channel::unbounded();
        let (tx, rx) = (LoggerTX(tx), LoggerRX(rx));

        // Set the logger through unshell
        crate::logger::set_logger_box(Box::new(tx));
        // Add the logger to the tree
        self.add_element(symbols::LOGGER.to_string(), Box::new(rx));
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
