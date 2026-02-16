//! Queue - A TreeElement wrapper around crossbeam channels for message queuing.

use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};

use crate::tree::symbols::{self, TYPE_QUEUE};
use crate::tree::TreeElement;

/// Generic queue wrapping crossbeam channels.
/// Provides Get, Poll, and GetLength commands via the tree interface.
pub struct Queue<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T> Queue<T> {
    pub fn new(sender: Sender<T>, receiver: Receiver<T>) -> Self {
        Self { sender, receiver }
    }

    pub fn channel() -> (Sender<T>, Self) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let queue = Self::new(tx.clone(), rx);
        (tx, queue)
    }

    pub fn sender(&self) -> &Sender<T> {
        &self.sender
    }

    pub fn len(&self) -> usize {
        self.receiver.len()
    }

    pub fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }

    pub fn try_recv(&self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    pub fn recv(&self) -> Option<T> {
        self.receiver.recv().ok()
    }
}

impl<T: serde::Serialize + Send> TreeElement for Queue<T> {
    fn get_type(&self) -> Value {
        json!(TYPE_QUEUE)
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match (target, message) {
            (Value::Null, Value::String(cmd)) => match cmd.as_ref() {
                symbols::CMD_GET => self.recv().map(|v| json!(v)).unwrap_or(json!(Value::Null)),
                symbols::CMD_POLL => self
                    .try_recv()
                    .map(|v| json!(v))
                    .unwrap_or(json!(Value::Null)),
                symbols::CMD_GET_LENGTH => json!(self.receiver.len()),
                _ => json!(symbols::ERR_INVALID_COMMAND),
            },
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}
