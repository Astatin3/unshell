//! TCP configuration structures for network components.

use serde::{Deserialize, Serialize};

/// Configuration for TCP client connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpClientConfig {
    /// Remote IP address or hostname
    pub address: String,
    /// Remote port number
    pub port: u16,
    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// Enable automatic reconnection
    #[serde(default)]
    pub auto_reconnect: bool,
    /// Reconnection delay in seconds
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay_secs: u64,
}

fn default_timeout() -> u64 {
    5000
}
fn default_reconnect_delay() -> u64 {
    5
}

impl Default for TcpClientConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_string(),
            port: 8080,
            timeout_ms: 5000,
            auto_reconnect: false,
            reconnect_delay_secs: 5,
        }
    }
}

impl TcpClientConfig {
    pub fn new(address: impl Into<String>, port: u16) -> Self {
        Self {
            address: address.into(),
            port,
            ..Default::default()
        }
    }
}

/// Configuration for TCP server listeners
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpServerConfig {
    /// Local IP address to bind to
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// Local port to listen on
    pub port: u16,
    /// Maximum number of concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}
fn default_max_connections() -> u32 {
    10
}

impl Default for TcpServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 10,
            timeout_ms: 5000,
        }
    }
}

impl TcpServerConfig {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            ..Default::default()
        }
    }

    pub fn bind_address(mut self, addr: impl Into<String>) -> Self {
        self.bind_address = addr.into();
        self
    }
}

/// Connection status information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub remote_address: Option<String>,
    pub local_address: Option<String>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connected_at: Option<u64>,
}

impl ConnectionStatus {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            remote_address: None,
            local_address: None,
            bytes_sent: 0,
            bytes_received: 0,
            connected_at: None,
        }
    }

    pub fn connected(remote: impl Into<String>, local: impl Into<String>) -> Self {
        Self {
            connected: true,
            remote_address: Some(remote.into()),
            local_address: Some(local.into()),
            bytes_sent: 0,
            bytes_received: 0,
            connected_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
        }
    }
}

/// Server listener status
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListenerStatus {
    pub listening: bool,
    pub bind_address: String,
    pub port: u16,
    pub active_connections: usize,
    pub total_connections: u64,
}

impl ListenerStatus {
    pub fn stopped(addr: impl Into<String>, port: u16) -> Self {
        Self {
            listening: false,
            bind_address: addr.into(),
            port,
            active_connections: 0,
            total_connections: 0,
        }
    }

    pub fn listening(addr: impl Into<String>, port: u16, connections: usize, total: u64) -> Self {
        Self {
            listening: true,
            bind_address: addr.into(),
            port,
            active_connections: connections,
            total_connections: total,
        }
    }
}
