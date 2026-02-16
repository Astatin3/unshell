//! TCP Client component for outbound connections.
//!
//! Provides a TreeElement for managing TCP client connections with
//! configuration, status queries, and reconnection support.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::tree::component::Component;
use crate::tree::symbols;
use crate::tree::tcp::config::{ConnectionStatus, TcpClientConfig};
use crate::tree::{Branch, TreeElement};

/// TCP Client component
pub struct TcpClient {
    name: String,
    config: TcpClientConfig,
    status: ConnectionStatus,
    stream: Option<Arc<Mutex<TcpStream>>>,
    branch: Branch,
}

impl TcpClient {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut branch = Branch::new("TCPClient");

        // Add internal state branch
        let state_branch = Branch::new("state");
        branch.add_child("state", Box::new(state_branch));

        Self {
            name: name.clone(),
            config: TcpClientConfig::default(),
            status: ConnectionStatus::disconnected(),
            stream: None,
            branch,
        }
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

    /// Send data over the connection
    pub fn send(&mut self, data: &[u8]) -> Result<usize, String> {
        let stream = self.stream.as_ref().ok_or("Not connected")?;

        let mut stream = stream.lock().map_err(|e| format!("Lock failed: {}", e))?;

        stream
            .write(data)
            .map_err(|e| format!("Write failed: {}", e))
    }

    /// Receive data from the connection
    pub fn recv(&mut self, buffer_size: usize) -> Result<Vec<u8>, String> {
        let stream = self.stream.as_ref().ok_or("Not connected")?;

        let mut stream = stream.lock().map_err(|e| format!("Lock failed: {}", e))?;

        let mut buffer = vec![0u8; buffer_size];
        let n = stream
            .read(&mut buffer)
            .map_err(|e| format!("Read failed: {}", e))?;

        buffer.truncate(n);
        Ok(buffer)
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
            "config": {
                "address": self.config.address,
                "port": self.config.port,
            }
        })
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
        self.config =
            serde_json::from_value(config).map_err(|e| format!("Invalid config: {}", e))?;
        self.connect()?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), String> {
        self.disconnect()
    }
}

impl TreeElement for TcpClient {
    fn get_type(&self) -> Value {
        json!("TCPClient")
    }

    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match target {
            Value::Null => {
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
                    } else {
                        json!(symbols::ERR_INVALID_COMMAND)
                    }
                } else {
                    json!(symbols::ERR_INVALID_COMMAND)
                }
            }
            Value::String(subtarget) => {
                match subtarget.as_str() {
                    "config" => {
                        // Return or modify configuration
                        json!(self.config)
                    }
                    "state" => {
                        // Return connection state
                        json!({
                            "connected": self.status.connected,
                            "remote": self.status.remote_address,
                        })
                    }
                    _ => json!(symbols::ERR_CHILD_NOT_FOUND),
                }
            }
            _ => json!(symbols::ERR_INVALID_TARGET),
        }
    }
}
