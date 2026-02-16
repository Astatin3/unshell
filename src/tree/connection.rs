//! Connection management for peer-to-peer communication between endpoints.
//! Uses crossbeam channels to simulate bidirectional TCP-like connections.

use std::collections::HashMap;

use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};

use crate::tree::symbols::{self, TYPE_CONNECTION, TYPE_CONNECTIONS};
use crate::tree::{Branch, TreeElement};

/// A bidirectional connection to another endpoint.
/// Wraps sender/receiver channels for message passing.
pub struct Connection {
    id: String,
    peer_id: String,
    sender: Sender<Value>,
    receiver: Receiver<Value>,
}

impl Connection {
    pub fn new(
        id: String,
        peer_id: String,
        sender: Sender<Value>,
        receiver: Receiver<Value>,
    ) -> Self {
        Self {
            id,
            peer_id,
            sender,
            receiver,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    pub fn send(&self, message: Value) {
        let _ = self.sender.send(message);
    }

    pub fn try_recv(&self) -> Option<Value> {
        self.receiver.try_recv().ok()
    }

    pub fn recv(&self) -> Option<Value> {
        self.receiver.recv().ok()
    }
}

impl TreeElement for Connection {
    fn get_type(&self) -> Value {
        json!(TYPE_CONNECTION)
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        "Send" => json!(symbols::ERR_MISSING_ARGS),
                        "Recv" => self.recv().unwrap_or(json!(Value::Null)),
                        "GetPeerId" => json!(self.peer_id),
                        symbols::CMD_GET_LENGTH => json!(0),
                        _ => json!(symbols::ERR_UNSUPPORTED_METHOD),
                    }
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            Value::String(cmd) if cmd == "Send" => json!(symbols::ERR_MISSING_ARGS),
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}

/// Container for managing multiple connections.
pub struct Connections {
    connections: HashMap<String, Connection>,
    branch: Branch,
}

impl Connections {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            branch: Branch::new(TYPE_CONNECTIONS),
        }
    }

    pub fn add(&mut self, id: String, connection: Connection) {
        self.connections.insert(id.clone(), connection);
        self.branch
            .add_child(id.clone(), Box::new(ConnectionStub { id }));
    }

    pub fn get(&mut self, id: &str) -> Option<&mut Connection> {
        self.connections.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Connection> {
        self.connections.remove(id)
    }

    pub fn branch(&self) -> &Branch {
        &self.branch
    }

    pub fn branch_mut(&mut self) -> &mut Branch {
        &mut self.branch
    }
}

impl Default for Connections {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeElement for Connections {
    fn get_type(&self) -> Value {
        self.branch.get_type()
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        self.branch.send_message(target, message)
    }
}

struct ConnectionStub {
    #[allow(dead_code)]
    id: String,
}

impl TreeElement for ConnectionStub {
    fn get_type(&self) -> Value {
        json!(TYPE_CONNECTION)
    }

    fn send_message(&mut self, _target: Value, _message: Value) -> Value {
        json!(symbols::ERR_UNSUPPORTED_METHOD)
    }
}

/// Creates a pair of connected channels for simulating TCP connections.
/// Returns ((sender_a, receiver_a), (sender_b, receiver_b)).
/// Messages sent on sender_a are received on receiver_b and vice versa.
pub fn create_channel_pair() -> (
    (Sender<Value>, Receiver<Value>),
    (Sender<Value>, Receiver<Value>),
) {
    let (tx1, rx1) = crossbeam_channel::unbounded::<Value>();
    let (tx2, rx2) = crossbeam_channel::unbounded::<Value>();
    ((tx1, rx2), (tx2, rx1))
}
