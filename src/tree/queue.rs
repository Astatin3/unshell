//! Queue - A TreeElement wrapper around crossbeam channels for message queuing.
//!
//! Provides a thread-safe queue that can be accessed via tree messages.
//! Useful for inter-thread communication and log buffering.
//!
//! # Tree Interface
//!
//! - `Get`: Receive one message (blocking)
//! - `Poll`: Try to receive without blocking
//! - `GetLength`: Get queue length
//!
//! # Usage
//!
//! ```rust
//! use unshell::tree::queue::Queue;
//! use unshell::tree::{Branch, TreeElement};
//! use serde_json::json;
//!
//! // Create queue with channel factory
//! let (sender, mut queue) = Queue::<String>::channel();
//!
//! // Add to branch
//! let mut branch = Branch::new("test");
//! branch.add_child("my_queue", Box::new(queue));
//!
//! // Send via sender (different thread)
//! sender.send("message".to_string()).unwrap();
//!
//! // Receive via tree message
//! let result = branch.send_message(json!("my_queue"), json!("Get"));
//! ```

use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};

use crate::tree::symbols::{self, TYPE_QUEUE};
use crate::tree::TreeElement;

/// Generic queue wrapping crossbeam channels.
///
/// Provides Get, Poll, and GetLength commands via the tree interface.
/// Thread-safe for multi-producer multi-consumer scenarios.
pub struct Queue<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
}

impl<T> Queue<T> {
    /// Create a new queue with given sender and receiver.
    pub fn new(sender: Sender<T>, receiver: Receiver<T>) -> Self {
        Self { sender, receiver }
    }

    /// Create a channel pair, returning sender and queue.
    ///
    /// Useful for setting up the queue where one end sends
    /// and the other end is exposed via tree.
    pub fn channel() -> (Sender<T>, Self) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let queue = Self::new(tx.clone(), rx);
        (tx, queue)
    }

    /// Get reference to the sender for producing messages.
    pub fn sender(&self) -> &Sender<T> {
        &self.sender
    }

    /// Get current queue length.
    pub fn len(&self) -> usize {
        self.receiver.len()
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }

    /// Try to receive without blocking.
    pub fn try_recv(&self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    /// Receive a message (blocking).
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
