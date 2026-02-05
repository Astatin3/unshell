/// Implement logging for the manager
use crossbeam_channel::Sender;

use crate::{
    logger::{Logger, Record},
    manager::Manager,
};

pub struct ManagerLogger {
    tx: Sender<Record>,
}

impl ManagerLogger {
    pub fn new(tx: Sender<Record>) -> Self {
        Self { tx }
    }
}

// impl Manager {
//     /// Initiate the unshell logger, piped through the manager
//     /// This will allow access to the logs through the tree
//     pub fn init_logger(&self) {
//         // Create the logger through the TX element of the manager
//         let logger = ManagerLogger::new(self.logs_tx.clone());

//         // Set the logger through unshell
//         crate::logger::set_logger_box(Box::new(logger));
//     }
// }

impl Logger for ManagerLogger {
    fn log(&self, log: crate::logger::Record) {
        // This will never panic if the program is operating properly
        self.tx.send(log).unwrap();
    }
}
