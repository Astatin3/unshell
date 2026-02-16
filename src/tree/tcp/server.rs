//! TCP Server component for inbound connections.
//!
//! Provides a TreeElement for managing TCP server listeners with
//! configuration, status queries, and connection management.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::tree::component::Component;
use crate::tree::symbols;
use crate::tree::tcp::config::{ListenerStatus, TcpServerConfig};
use crate::tree::{Branch, TreeElement};

/// A connected client managed by the server
struct ManagedClient {
    id: String,
    stream: TcpStream,
    peer_addr: String,
}

impl ManagedClient {
    fn new(id: String, stream: TcpStream) -> Self {
        let peer_addr = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            id,
            stream,
            peer_addr,
        }
    }

    fn send(&mut self, data: &[u8]) -> Result<usize, String> {
        self.stream
            .write(data)
            .map_err(|e| format!("Write failed: {}", e))
    }

    fn recv(&mut self, buffer_size: usize) -> Result<Vec<u8>, String> {
        let mut buffer = vec![0u8; buffer_size];
        let _ = self.stream.set_read_timeout(Some(Duration::from_secs(1)));

        match self.stream.read(&mut buffer) {
            Ok(0) => Err("Connection closed".to_string()),
            Ok(n) => {
                buffer.truncate(n);
                Ok(buffer)
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(vec![]),
            Err(e) => Err(format!("Read failed: {}", e)),
        }
    }

    fn peer_address(&self) -> &str {
        &self.peer_addr
    }
}

/// TCP Server component
pub struct TcpServer {
    name: String,
    config: TcpServerConfig,
    status: ListenerStatus,
    listener: Option<TcpListener>,
    clients: HashMap<String, Arc<Mutex<ManagedClient>>>,
    total_connections: u64,
    branch: Branch,
}

impl TcpServer {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();

        Self {
            name: name.clone(),
            config: TcpServerConfig::default(),
            status: ListenerStatus::stopped("0.0.0.0", 0),
            listener: None,
            clients: HashMap::new(),
            total_connections: 0,
            branch: Branch::new("TCPServer"),
        }
    }

    /// Start listening for connections
    pub fn listen(&mut self) -> Result<(), String> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.port);

        let listener = TcpListener::bind(&addr).map_err(|e| format!("Bind failed: {}", e))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        self.listener = Some(listener);
        self.status = ListenerStatus::listening(
            &self.config.bind_address,
            self.config.port,
            0,
            self.total_connections,
        );

        Ok(())
    }

    /// Stop listening
    pub fn stop(&mut self) -> Result<(), String> {
        self.listener = None;
        self.clients.clear();
        self.status = ListenerStatus::stopped(&self.config.bind_address, self.config.port);
        Ok(())
    }

    /// Accept a new connection (non-blocking)
    pub fn accept(&mut self) -> Option<(String, TcpStream)> {
        let listener = self.listener.as_ref()?;

        match listener.accept() {
            Ok((stream, _addr)) => {
                self.total_connections += 1;
                let id = format!("conn-{}", self.total_connections);
                Some((id, stream))
            }
            Err(_) => None,
        }
    }

    /// Register an accepted connection
    pub fn register_client(&mut self, id: String, stream: TcpStream) {
        let client_id = id.clone();
        self.clients.insert(
            id,
            Arc::new(Mutex::new(ManagedClient::new(client_id, stream))),
        );
    }

    /// Disconnect a client
    pub fn disconnect_client(&mut self, id: &str) -> Result<(), String> {
        self.clients
            .remove(id)
            .ok_or_else(|| format!("Client '{}' not found", id))?;
        Ok(())
    }

    /// Send to a specific client
    pub fn send_to(&mut self, client_id: &str, data: &[u8]) -> Result<usize, String> {
        let client = self
            .clients
            .get(client_id)
            .ok_or_else(|| format!("Client '{}' not found", client_id))?;

        let mut client = client.lock().map_err(|e| format!("Lock failed: {}", e))?;

        client.send(data)
    }

    /// Check if listening
    pub fn is_listening(&self) -> bool {
        self.status.listening
    }

    /// Get current configuration
    pub fn config(&self) -> &TcpServerConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut TcpServerConfig {
        &mut self.config
    }

    /// Get status as JSON
    pub fn get_status(&self) -> Value {
        let client_list: Vec<Value> = self.clients.keys().map(|k| json!(k)).collect();

        json!({
            "listening": self.status.listening,
            "bind_address": self.config.bind_address,
            "port": self.config.port,
            "active_connections": self.clients.len(),
            "total_connections": self.total_connections,
            "clients": client_list,
        })
    }
}

impl Component for TcpServer {
    fn name(&self) -> &str {
        &self.name
    }

    fn status(&self) -> Value {
        self.get_status()
    }

    fn init(&mut self, config: Value) -> Result<(), String> {
        self.config =
            serde_json::from_value(config).map_err(|e| format!("Invalid config: {}", e))?;
        self.listen()?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), String> {
        self.stop()
    }
}

impl TreeElement for TcpServer {
    fn get_type(&self) -> Value {
        json!("TCPServer")
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        "Listen" | "Start" => match self.listen() {
                            Ok(_) => json!({"success": true}),
                            Err(e) => json!({"success": false, "error": e}),
                        },
                        "Stop" => match self.stop() {
                            Ok(_) => json!({"success": true}),
                            Err(e) => json!({"success": false, "error": e}),
                        },
                        "Status" => self.get_status(),
                        symbols::CMD_GET_CHILDREN => {
                            let children = self
                                .branch
                                .children()
                                .keys()
                                .map(|k| json!(k))
                                .collect::<Vec<_>>();
                            json!(children)
                        }
                        _ => json!(symbols::ERR_UNSUPPORTED_METHOD),
                    }
                } else if let Value::Object(obj) = message {
                    if let Some(config) = obj.get("config") {
                        match serde_json::from_value(config.clone()) {
                            Ok(cfg) => {
                                self.config = cfg;
                                json!({"success": true})
                            }
                            Err(e) => json!({"success": false, "error": e.to_string()}),
                        }
                    } else {
                        json!(symbols::ERR_INVALID_COMMAND)
                    }
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            Value::String(subtarget) => match subtarget.as_str() {
                "config" => json!(self.config),
                "status" => self.get_status(),
                _ => json!(symbols::ERR_CHILD_NOT_FOUND),
            },
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}
