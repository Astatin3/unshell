//! EndpointManager - Root element for endpoint management.
//!
//! Provides a standardized tree structure for all endpoints with:
//! - id: Read-only endpoint identifier
//! - logs: Queue for log messages
//! - connections: Container for peer connections
//! - components: Extensible component system (accessed via tree messages)

use std::collections::HashMap;

use crossbeam_channel::{Receiver, Sender};
use serde_json::{json, Value};

use crate::tree::component::ComponentRegistry;
use crate::tree::queue::Queue;
use crate::tree::readonly::ReadOnly;
use crate::tree::symbols::{self, TYPE_CONNECTION, TYPE_ENDPOINT};
use crate::tree::{Branch, TreeElement};

pub(crate) struct Connection {
    id: String,
    peer_id: String,
    sender: Sender<Value>,
    receiver: Receiver<Value>,
}

impl Connection {
    pub(crate) fn new(
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

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn send(&self, message: Value) {
        let _ = self.sender.send(message);
    }

    pub(crate) fn recv(&self) -> Option<Value> {
        self.receiver.recv().ok()
    }
}

impl TreeElement for Connections {
    fn get_type(&self) -> Value {
        json!(symbols::TYPE_CONNECTIONS)
    }

    fn send_message(&mut self, _target: Value, _message: Value) -> Value {
        json!(symbols::ERR_UNSUPPORTED_METHOD)
    }
}

pub(crate) struct Connections {
    connections: HashMap<String, Connection>,
    branch: Branch,
}

impl Connections {
    pub(crate) fn new() -> Self {
        Self {
            connections: HashMap::new(),
            branch: Branch::new(symbols::TYPE_CONNECTIONS),
        }
    }

    pub(crate) fn add(&mut self, id: String, connection: Connection) {
        self.connections.insert(id.clone(), connection);
    }
}

pub(crate) fn create_channel_pair() -> (
    (Sender<Value>, Receiver<Value>),
    (Sender<Value>, Receiver<Value>),
) {
    let (tx1, rx1) = crossbeam_channel::unbounded::<Value>();
    let (tx2, rx2) = crossbeam_channel::unbounded::<Value>();
    ((tx1, rx2), (tx2, rx1))
}

pub struct EndpointManager {
    branch: Branch,
    logs_sender: Sender<Value>,
}

impl EndpointManager {
    pub fn new(endpoint_id: impl Into<String>) -> Self {
        let endpoint_id = endpoint_id.into();

        let (logs_sender, logs_receiver) = crossbeam_channel::unbounded();
        let logs = Queue::new(logs_sender.clone(), logs_receiver);

        let connections = Connections::new();
        let components = ComponentRegistry::new();

        let mut branch = Branch::new(TYPE_ENDPOINT);
        branch.add_child("id", Box::new(ReadOnly::new(&endpoint_id, TYPE_ENDPOINT)));
        branch.add_child("logs", Box::new(logs));
        branch.add_child("connections", Box::new(connections));
        branch.add_child("components", Box::new(components));

        Self {
            branch,
            logs_sender,
        }
    }

    pub fn logs_sender(&self) -> &Sender<Value> {
        &self.logs_sender
    }

    pub fn branch(&self) -> &Branch {
        &self.branch
    }

    pub fn branch_mut(&mut self) -> &mut Branch {
        &mut self.branch
    }

    pub fn add_connection(&mut self, id: String, peer_id: String) -> Connection {
        let ((tx_local, rx_remote), (tx_remote, rx_local)) = create_channel_pair();

        let conn_a = Connection::new(id.clone(), peer_id.clone(), tx_remote, rx_local);
        let conn_b = Connection::new(id.clone(), peer_id, tx_local, rx_remote);

        if let Some(connections) = self.branch.get_child("connections") {
            let _ = connections.send_message(Value::String(id), json!({ "Add": conn_a.id() }));
        }

        conn_b
    }
}

impl TreeElement for EndpointManager {
    fn get_type(&self) -> Value {
        json!(TYPE_ENDPOINT)
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        self.branch.send_message(target, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_endpoint_id() {
        let mut endpoint = EndpointManager::new("test-endpoint-1");

        let response = endpoint.branch.send_message(json!("id"), json!(null));

        assert_eq!(response, json!("test-endpoint-1"));
    }

    #[test]
    fn test_endpoint_get_children() {
        let mut endpoint = EndpointManager::new("test-endpoint");

        let response = endpoint
            .branch
            .send_message(json!(null), json!("GetChildren"));

        let children = response.as_object().unwrap();
        assert!(children.contains_key("id"));
        assert!(children.contains_key("logs"));
        assert!(children.contains_key("connections"));
    }

    #[test]
    fn test_logs_queue() {
        let mut endpoint = EndpointManager::new("test-endpoint");
        let sender = endpoint.logs_sender.clone();

        sender.send(json!("log message 1")).unwrap();
        sender.send(json!("log message 2")).unwrap();

        let response = endpoint
            .branch
            .send_message(json!("logs"), json!("GetLength"));

        assert_eq!(response, json!(2));
    }

    #[test]
    fn test_logs_read() {
        let mut endpoint = EndpointManager::new("test-endpoint");
        let sender = endpoint.logs_sender.clone();

        sender.send(json!("log message 1")).unwrap();
        sender.send(json!("log message 2")).unwrap();

        let response1 = endpoint.branch.send_message(json!("logs"), json!("Get"));
        let response2 = endpoint.branch.send_message(json!("logs"), json!("Get"));

        assert_eq!(response1, json!("log message 1"));
        assert_eq!(response2, json!("log message 2"));
    }

    #[test]
    fn test_simulated_tcp_connection() {
        let ((tx_a_to_b, rx_a_to_b), (tx_b_to_a, rx_b_to_a)) = create_channel_pair();

        let conn_a = Connection::new(
            "conn-1".to_string(),
            "endpoint-b".to_string(),
            tx_a_to_b,
            rx_b_to_a,
        );

        let conn_b = Connection::new(
            "conn-1".to_string(),
            "endpoint-a".to_string(),
            tx_b_to_a,
            rx_a_to_b,
        );

        conn_a.send(json!("Hello from A"));

        let response = conn_b.recv();
        assert_eq!(response, Some(json!("Hello from A")));
    }

    #[test]
    fn test_pivot_routing() {
        let mut endpoint_gateway = EndpointManager::new("gateway");

        let response = endpoint_gateway
            .branch
            .send_message(json!(["connections", "internal"]), json!("GetChildren"));

        assert!(response.is_string());
    }
}
