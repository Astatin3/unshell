//! TCP Client component for outbound connections.
//!
//! Provides a TreeElement for managing TCP client connections with
//! configuration, status queries, reconnection support, and protocol stacking.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::protocols::{ProtocolConfig, ProtocolStack};
use crate::tcp::config::{ConnectionStatus, TcpClientConfig};
use unshell::tree::component::Component;
use unshell::tree::message::TreeMessage;
use unshell::tree::symbols;
use unshell::tree::{Branch, TreeElement};

/// TCP Client component with protocol stacking support.
///
/// This component can:
/// - Connect to remote TCP servers
/// - Apply protocol stacks (base64, http, etc.)
/// - Send/receive messages via RPC
/// - Auto-reconnect on failure
#[derive(Debug, Serialize, Deserialize)]
pub struct TcpClient {
    /// Unique name for this client
    pub name: String,
    /// Connection configuration
    pub config: TcpClientConfig,
    /// Protocol stack configuration
    #[serde(default)]
    pub protocols: Vec<ProtocolConfig>,
    /// Current connection status
    #[serde(skip)]
    status: ConnectionStatus,
    /// Active TCP stream
    #[serde(skip)]
    stream: Option<Arc<Mutex<TcpStream>>>,
    /// Protocol stack (runtime)
    #[serde(skip)]
    protocol_stack: ProtocolStack,
    /// Internal tree structure
    #[serde(skip)]
    branch: Branch,
}

impl TcpClient {
    /// Create a new TCP client with default settings
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(name, TcpClientConfig::default())
    }

    /// Create a new TCP client with custom configuration
    pub fn with_config(name: impl Into<String>, config: TcpClientConfig) -> Self {
        let name = name.into();

        let mut branch = Branch::new("TCPClient");
        let state_branch = Branch::new("state");
        branch.add_child("state", Box::new(state_branch));

        Self {
            name: name.clone(),
            config,
            protocols: Vec::new(),
            status: ConnectionStatus::disconnected(),
            stream: None,
            protocol_stack: ProtocolStack::new(),
            branch,
        }
    }

    /// Set protocol stack configuration
    pub fn set_protocols(&mut self, protocols: Vec<ProtocolConfig>) -> Result<(), String> {
        self.protocols = protocols.clone();
        self.protocol_stack = ProtocolStack::from_configs(&protocols).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Connect to the configured address
    pub fn connect(&mut self) -> Result<(), String> {
        let addr = format!("{}:{}", self.config.address, self.config.port);

        let stream = TcpStream::connect_timeout(
            &addr
                .parse()
                .map_err(|e| format!("Invalid address: {}", e))?,
            Duration::from_millis(self.config.timeout_ms),
        )
        .map_err(|e| format!("Connection failed: {}", e))?;

        stream
            .set_nonblocking(false)
            .map_err(|e| format!("Failed to set blocking: {}", e))?;

        let local = stream
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let remote = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();

        self.status = ConnectionStatus::connected(remote, local);
        self.stream = Some(Arc::new(Mutex::new(stream)));

        Ok(())
    }

    /// Disconnect from server
    pub fn disconnect(&mut self) -> Result<(), String> {
        self.stream = None;
        self.status = ConnectionStatus::disconnected();
        Ok(())
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.status.connected
    }

    /// Send raw data over the connection
    pub fn send_raw(&mut self, data: &[u8]) -> Result<usize, String> {
        let stream = self.stream.as_ref().ok_or("Not connected")?;
        let mut stream = stream.lock().map_err(|e| format!("Lock failed: {}", e))?;
        stream
            .write(data)
            .map_err(|e| format!("Write failed: {}", e))
    }

    /// Receive raw data from the connection
    pub fn recv_raw(&mut self, buffer_size: usize) -> Result<Vec<u8>, String> {
        let stream = self.stream.as_ref().ok_or("Not connected")?;
        let mut stream = stream.lock().map_err(|e| format!("Lock failed: {}", e))?;
        let mut buffer = vec![0u8; buffer_size];
        let n = stream
            .read(&mut buffer)
            .map_err(|e| format!("Read failed: {}", e))?;
        buffer.truncate(n);
        Ok(buffer)
    }

    /// Send a TreeMessage through the protocol stack
    pub fn send_message_raw(&mut self, message: &TreeMessage) -> Result<(), String> {
        let encoded = self
            .protocol_stack
            .encode_message(message)
            .map_err(|e| format!("Encoding failed: {}", e))?;

        self.send_raw(&encoded)?;
        Ok(())
    }

    /// Receive and decode a TreeMessage
    pub fn recv_message(&mut self, buffer_size: usize) -> Result<TreeMessage, String> {
        let data = self.recv_raw(buffer_size)?;

        self.protocol_stack
            .decode_message(&data)
            .map_err(|e| format!("Decoding failed: {}", e))
    }

    /// Send and wait for response (RPC pattern)
    pub fn rpc_call(&mut self, message: &TreeMessage) -> Result<TreeMessage, String> {
        let id = message.id.clone();

        self.send_message_raw(message)?;

        // Simple blocking receive - in production would have timeout
        let response = self.recv_message(4096)?;

        // Verify it's a response to our message
        if let Some(response_to) = &response.response_to {
            if response_to != id.as_deref().unwrap_or("") {
                return Err("Response ID mismatch".to_string());
            }
        }

        Ok(response)
    }

    /// Get current configuration
    pub fn config(&self) -> &TcpClientConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut TcpClientConfig {
        &mut self.config
    }

    /// Get status as JSON
    pub fn get_status(&self) -> Value {
        json!({
            "connected": self.status.connected,
            "remote_address": self.status.remote_address,
            "local_address": self.status.local_address,
            "bytes_sent": self.status.bytes_sent,
            "bytes_received": self.status.bytes_received,
            "config": self.config,
            "protocols": self.protocol_stack.to_configs(),
        })
    }

    /// Handle RPC call from message
    fn handle_rpc(&mut self, payload: &Value) -> Value {
        let method = match payload.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => return json!({"success": false, "error": "missing method"}),
        };

        let params = payload.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "connect" => {
                // Allow override of address/port
                if let Some(addr) = params.get("address").and_then(|a| a.as_str()) {
                    self.config.address = addr.to_string();
                }
                if let Some(port) = params.get("port").and_then(|p| p.as_u64()) {
                    self.config.port = port as u16;
                }

                match self.connect() {
                    Ok(_) => json!({"success": true, "status": self.status}),
                    Err(e) => json!({"success": false, "error": e}),
                }
            }
            "disconnect" => match self.disconnect() {
                Ok(_) => json!({"success": true}),
                Err(e) => json!({"success": false, "error": e}),
            },
            "send" => {
                let data = params
                    .get("data")
                    .and_then(|d| d.as_str())
                    .map(|s| s.as_bytes().to_vec());

                match data {
                    Some(data) => match self.send_raw(&data) {
                        Ok(n) => json!({"success": true, "bytes_sent": n}),
                        Err(e) => json!({"success": false, "error": e}),
                    },
                    None => json!({"success": false, "error": "missing data"}),
                }
            }
            "recv" => {
                let size = params
                    .get("size")
                    .and_then(|s| s.as_u64())
                    .map(|s| s as usize)
                    .unwrap_or(4096);
                match self.recv_raw(size) {
                    Ok(data) => json!({
                        "success": true,
                        "data": String::from_utf8_lossy(&data),
                        "bytes": data.len()
                    }),
                    Err(e) => json!({"success": false, "error": e}),
                }
            }
            "status" => self.get_status(),
            "set_protocols" => {
                if let Some(protocols) = params.get("protocols") {
                    match serde_json::from_value(protocols.clone()) {
                        Ok(p) => match self.set_protocols(p) {
                            Ok(_) => json!({"success": true}),
                            Err(e) => json!({"success": false, "error": e}),
                        },
                        Err(e) => json!({"success": false, "error": e.to_string()}),
                    }
                } else {
                    json!({"success": false, "error": "missing protocols"})
                }
            }
            _ => json!({"success": false, "error": format!("unknown method: {}", method)}),
        }
    }
}

impl Component for TcpClient {
    fn name(&self) -> &str {
        &self.name
    }

    fn status(&self) -> Value {
        self.get_status()
    }

    fn init(&mut self, config: Value) -> Result<(), String> {
        // Support both legacy config and new format
        if let Some(client_config) = config.get("config") {
            self.config = serde_json::from_value(client_config.clone())
                .map_err(|e| format!("Invalid config: {}", e))?;
        } else {
            self.config = serde_json::from_value(config.clone())
                .map_err(|e| format!("Invalid config: {}", e))?;
        }

        if let Some(protocols) = config.get("protocols") {
            let p: Vec<ProtocolConfig> = serde_json::from_value(protocols.clone())
                .map_err(|e| format!("Invalid protocols: {}", e))?;
            self.set_protocols(p)?;
        }

        self.connect()?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), String> {
        self.disconnect()
    }
}

impl TreeElement for TcpClient {
    fn get_type(&self) -> Value {
        json!({
            "type": "TCPClient",
            "name": self.name,
        })
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
                // Check for RPC call format
                if message.get("method").is_some() {
                    return self.handle_rpc(&message);
                }

                // Legacy string commands
                if let Some(cmd) = message.as_str() {
                    match cmd {
                        "Connect" => match self.connect() {
                            Ok(_) => json!({"success": true}),
                            Err(e) => json!({"success": false, "error": e}),
                        },
                        "Disconnect" => match self.disconnect() {
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
                    // Handle configuration changes
                    if let Some(config) = obj.get("config") {
                        match serde_json::from_value(config.clone()) {
                            Ok(cfg) => {
                                self.config = cfg;
                                json!({"success": true})
                            }
                            Err(e) => json!({"success": false, "error": e.to_string()}),
                        }
                    } else if obj.get("method").is_some() {
                        let payload = Value::Object(obj.clone());
                        self.handle_rpc(&payload)
                    } else {
                        json!(symbols::ERR_INVALID_COMMAND)
                    }
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            Value::String(subtarget) => match subtarget.as_str() {
                "config" => json!(self.config),
                "state" => json!({
                    "connected": self.status.connected,
                    "remote": self.status.remote_address,
                }),
                "protocols" => json!(self.protocol_stack.to_configs()),
                _ => json!(symbols::ERR_CHILD_NOT_FOUND),
            },
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = TcpClient::new("test-client");
        assert_eq!(client.name(), "test-client");
        assert!(!client.is_connected());
    }

    #[test]
    fn test_config_serialization() {
        let client = TcpClient::with_config("test", TcpClientConfig::new("127.0.0.1", 8080));
        let json = serde_json::to_string(&client).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("127.0.0.1"));
    }

    #[test]
    fn test_rpc_status() {
        let mut client = TcpClient::new("test");
        let result = client.send_message(json!(null), json!({"method": "status"}));

        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("connected"));
    }
}
