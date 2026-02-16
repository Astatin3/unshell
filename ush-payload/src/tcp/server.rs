//! TCP Server component for inbound connections.
//!
//! Provides a TreeElement for managing TCP server listeners with
//! configuration, status queries, connection management, and protocol stacking.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::protocols::{ProtocolConfig, ProtocolStack};
use crate::tcp::config::{ListenerStatus, TcpServerConfig};
use unshell::tree::component::Component;
use unshell::tree::message::TreeMessage;
use unshell::tree::symbols::*;
use unshell::tree::{Branch, TreeElement};

/// A connected client managed by the server
#[derive(Debug)]
pub struct ManagedClient {
    pub id: String,
    stream: TcpStream,
    peer_addr: String,
    local_addr: String,
}

impl ManagedClient {
    pub fn new(id: String, stream: TcpStream) -> Self {
        let peer_addr = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let local_addr = stream
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            id,
            stream,
            peer_addr,
            local_addr,
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<usize, String> {
        self.stream
            .write(data)
            .map_err(|e| format!("Write failed: {}", e))
    }

    pub fn recv(&mut self, buffer_size: usize) -> Result<Vec<u8>, String> {
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

    pub fn peer_address(&self) -> &str {
        &self.peer_addr
    }

    pub fn local_address(&self) -> &str {
        &self.local_addr
    }

    pub fn set_nonblocking(&mut self, nonblocking: bool) -> Result<(), String> {
        self.stream
            .set_nonblocking(nonblocking)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))
    }
}

/// TCP Server component with protocol stacking support.
///
/// This component can:
/// - Listen for incoming TCP connections
/// - Manage multiple concurrent connections
/// - Apply protocol stacks to connections
/// - Send/receive messages via RPC
#[derive(Debug, Serialize, Deserialize)]
pub struct TcpServer {
    /// Unique name for this server
    pub name: String,
    /// Server configuration
    pub config: TcpServerConfig,
    /// Protocol stack for incoming connections
    #[serde(default)]
    pub protocols: Vec<ProtocolConfig>,
    /// Current listener status
    #[serde(skip)]
    status: ListenerStatus,
    /// TCP listener (runtime only)
    #[serde(skip)]
    listener: Option<TcpListener>,
    /// Active clients
    #[serde(skip)]
    clients: HashMap<String, Arc<Mutex<ManagedClient>>>,
    /// Protocol stacks per client
    #[serde(skip)]
    client_protocols: HashMap<String, ProtocolStack>,
    /// Total connections since start
    total_connections: u64,
    /// Internal tree structure
    #[serde(skip)]
    branch: Branch,
}

impl TcpServer {
    /// Create a new TCP server with default settings
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(name, TcpServerConfig::default())
    }

    /// Create a new TCP server with custom configuration
    pub fn with_config(name: impl Into<String>, config: TcpServerConfig) -> Self {
        let name = name.into();

        Self {
            name: name.clone(),
            config,
            protocols: Vec::new(),
            status: ListenerStatus::stopped(STR_0_0_0_0, 0),
            listener: None,
            clients: HashMap::new(),
            client_protocols: HashMap::new(),
            total_connections: 0,
            branch: Branch::new(TYPE_TCP_SERVER),
        }
    }

    /// Set protocol stack configuration
    pub fn set_protocols(&mut self, protocols: Vec<ProtocolConfig>) -> Result<(), String> {
        self.protocols = protocols.clone();
        // Don't rebuild client_protocols here - each client gets its own stack
        Ok(())
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
        self.client_protocols.clear();
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

        // Create protocol stack for this client
        let mut protocol_stack = ProtocolStack::new();
        for config in &self.protocols {
            let _ = protocol_stack.push(config);
        }

        let client = Arc::new(Mutex::new(ManagedClient::new(client_id, stream)));
        self.clients.insert(id.clone(), client);
        self.client_protocols.insert(id, protocol_stack);
    }

    /// Disconnect a client
    pub fn disconnect_client(&mut self, id: &str) -> Result<(), String> {
        self.clients
            .remove(id)
            .ok_or_else(|| format!("Client '{}' not found", id))?;
        self.client_protocols.remove(id);
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

    /// Receive from a specific client
    pub fn recv_from(&mut self, client_id: &str, buffer_size: usize) -> Result<Vec<u8>, String> {
        let client = self
            .clients
            .get(client_id)
            .ok_or_else(|| format!("Client '{}' not found", client_id))?;

        let mut client = client.lock().map_err(|e| format!("Lock failed: {}", e))?;
        client.recv(buffer_size)
    }

    /// Send TreeMessage to client through protocol stack
    pub fn send_message_to(
        &mut self,
        client_id: &str,
        message: &TreeMessage,
    ) -> Result<(), String> {
        let protocol_stack = self
            .client_protocols
            .get_mut(client_id)
            .ok_or_else(|| format!("Client '{}' not found", client_id))?;

        let encoded = protocol_stack
            .encode_message(message)
            .map_err(|e| format!("Encoding failed: {}", e))?;

        self.send_to(client_id, &encoded).map(|_| ())
    }

    /// Receive TreeMessage from client through protocol stack
    pub fn recv_message_from(
        &mut self,
        client_id: &str,
        buffer_size: usize,
    ) -> Result<TreeMessage, String> {
        let data = self.recv_from(client_id, buffer_size)?;

        let protocol_stack = self
            .client_protocols
            .get_mut(client_id)
            .ok_or_else(|| format!("Client '{}' not found", client_id))?;

        protocol_stack
            .decode_message(&data)
            .map_err(|e| format!("Decoding failed: {}", e))
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
        let client_list: Vec<Value> = self
            .clients
            .iter()
            .map(|(id, client)| {
                let addr = client
                    .lock()
                    .map(|c| c.peer_address().to_string())
                    .unwrap_or_else(|_| STR_UNKNOWN.to_string());
                json!({KEY_ID: id, KEY_PEER: addr})
            })
            .collect();

        json!({
            KEY_LISTENING: self.status.listening,
            KEY_BIND_ADDRESS: self.config.bind_address,
            KEY_PORT: self.config.port,
            KEY_ACTIVE_CONNECTIONS: self.clients.len(),
            KEY_TOTAL_CONNECTIONS: self.total_connections,
            KEY_CONFIG: self.config,
            KEY_PROTOCOLS: self.protocols,
            KEY_CLIENTS: client_list,
        })
    }

    /// Handle RPC call from message
    fn handle_rpc(&mut self, payload: &Value) -> Value {
        let method = match payload.get(KEY_METHOD).and_then(|m| m.as_str()) {
            Some(m) => m,
            None => return json!({KEY_SUCCESS: false, KEY_ERROR: ERR_MISSING_METHOD}),
        };

        let params = payload.get(KEY_PARAMS).cloned().unwrap_or(Value::Null);

        match method {
            METHOD_LISTEN | METHOD_START => {
                if let Some(addr) = params.get(KEY_BIND_ADDRESS).and_then(|a| a.as_str()) {
                    self.config.bind_address = addr.to_string();
                }
                if let Some(port) = params.get(KEY_PORT).and_then(|p| p.as_u64()) {
                    self.config.port = port as u16;
                }

                match self.listen() {
                    Ok(_) => json!({KEY_SUCCESS: true, KEY_STATUS: self.status}),
                    Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
                }
            }
            METHOD_STOP => match self.stop() {
                Ok(_) => json!({KEY_SUCCESS: true}),
                Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
            },
            METHOD_ACCEPT => {
                if let Some((id, stream)) = self.accept() {
                    self.register_client(id.clone(), stream);
                    json!({KEY_SUCCESS: true, KEY_CLIENT_ID: id})
                } else {
                    json!({KEY_SUCCESS: true, KEY_CLIENT_ID: serde_json::Value::Null})
                }
            }
            METHOD_SEND => {
                let client_id = params
                    .get(KEY_CLIENT_ID)
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| json!({KEY_ERROR: ERR_MISSING_CLIENT_ID}));

                match client_id {
                    Ok(id) => {
                        let data = params
                            .get(KEY_DATA)
                            .and_then(|d| d.as_str())
                            .map(|s| s.as_bytes().to_vec());

                        match data {
                            Some(data) => match self.send_to(id, &data) {
                                Ok(n) => json!({KEY_SUCCESS: true, KEY_BYTES_SENT: n}),
                                Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
                            },
                            None => json!({KEY_SUCCESS: false, KEY_ERROR: ERR_MISSING_DATA}),
                        }
                    }
                    Err(e) => e,
                }
            }
            METHOD_RECV => {
                let client_id = params
                    .get(KEY_CLIENT_ID)
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| json!({KEY_ERROR: ERR_MISSING_CLIENT_ID}));

                match client_id {
                    Ok(id) => {
                        let size = params
                            .get(KEY_SIZE)
                            .and_then(|s| s.as_u64())
                            .map(|s| s as usize)
                            .unwrap_or(4096);
                        match self.recv_from(id, size) {
                            Ok(data) => json!({
                                KEY_SUCCESS: true,
                                KEY_DATA: String::from_utf8_lossy(&data),
                                "bytes": data.len()
                            }),
                            Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
                        }
                    }
                    Err(e) => e,
                }
            }
            METHOD_DISCONNECT => {
                let client_id = params
                    .get(KEY_CLIENT_ID)
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| json!({KEY_ERROR: ERR_MISSING_CLIENT_ID}));

                match client_id {
                    Ok(id) => match self.disconnect_client(id) {
                        Ok(_) => json!({KEY_SUCCESS: true}),
                        Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
                    },
                    Err(e) => e,
                }
            }
            METHOD_STATUS => self.get_status(),
            METHOD_LIST_CLIENTS => {
                let clients: Vec<Value> = self.clients.keys().map(|k| json!(k)).collect();
                json!({KEY_SUCCESS: true, KEY_CLIENTS: clients})
            }
            _ => json!({KEY_SUCCESS: false, KEY_ERROR: format!("unknown method: {}", method)}),
        }
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
        if let Some(server_config) = config.get(KEY_CONFIG) {
            self.config = serde_json::from_value(server_config.clone())
                .map_err(|e| format!("Invalid config: {}", e))?;
        } else {
            self.config = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid config: {}", e))?;
        }

        if let Some(protocols) = config.get(KEY_PROTOCOLS) {
            let p: Vec<ProtocolConfig> = serde_json::from_value(protocols.clone())
                .map_err(|e| format!("Invalid protocols: {}", e))?;
            self.set_protocols(p)?;
        }

        self.listen()?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), String> {
        self.stop()
    }
}

impl TreeElement for TcpServer {
    fn get_type(&self) -> Value {
        json!({
            KEY_TYPE: TYPE_TCP_SERVER,
            KEY_NAME: self.name,
        })
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                if message.get(KEY_METHOD).is_some() {
                    return self.handle_rpc(&message);
                }

                if let Some(cmd) = message.as_str() {
                    match cmd {
                        CMD_LISTEN | CMD_START => match self.listen() {
                            Ok(_) => json!({KEY_SUCCESS: true}),
                            Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
                        },
                        CMD_STOP => match self.stop() {
                            Ok(_) => json!({KEY_SUCCESS: true}),
                            Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e}),
                        },
                        CMD_STATUS => self.get_status(),
                        CMD_GET_CHILDREN => {
                            let children = self
                                .branch
                                .children()
                                .keys()
                                .map(|k| json!(k))
                                .collect::<Vec<_>>();
                            json!(children)
                        }
                        _ => json!(ERR_UNSUPPORTED_METHOD),
                    }
                } else if let Value::Object(obj) = message {
                    if let Some(config) = obj.get(KEY_CONFIG) {
                        match serde_json::from_value(config.clone()) {
                            Ok(cfg) => {
                                self.config = cfg;
                                json!({KEY_SUCCESS: true})
                            }
                            Err(e) => json!({KEY_SUCCESS: false, KEY_ERROR: e.to_string()}),
                        }
                    } else if obj.get(KEY_METHOD).is_some() {
                        let payload = Value::Object(obj.clone());
                        self.handle_rpc(&payload)
                    } else {
                        json!(ERR_INVALID_COMMAND)
                    }
                } else {
                    json!(ERR_INVALID_COMMAND)
                }
            }
            Value::String(subtarget) => match subtarget.as_str() {
                STR_CONFIG => json!(self.config),
                KEY_STATUS => self.get_status(),
                KEY_CLIENTS => {
                    let clients: Vec<Value> = self.clients.keys().map(|k| json!(k)).collect();
                    json!(clients)
                }
                _ => json!(ERR_CHILD_NOT_FOUND),
            },
            _ => json!(ERR_INVALID_TARGET),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = TcpServer::new("test-server");
        assert_eq!(server.name(), "test-server");
        assert!(!server.is_listening());
    }

    #[test]
    fn test_config_serialization() {
        let server = TcpServer::with_config("test", TcpServerConfig::new(8080));
        let json = serde_json::to_string(&server).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("8080"));
    }

    #[test]
    fn test_rpc_status() {
        let mut server = TcpServer::new("test");
        let result = server.send_message(json!(null), json!({"method": "status"}));

        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("listening"));
    }
}
