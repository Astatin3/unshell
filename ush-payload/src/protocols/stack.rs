//! Protocol stack implementation for layered network communication.
//!
//! The stack processes protocols from outermost (closest to app) to innermost (closest to network).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::base64::{Base64Protocol, IdentityProtocol};
use super::http::HttpProtocol;
use unshell::tree::message::TreeMessage;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Encoding failed: {0}")]
    EncodeError(String),
    #[error("Decoding failed: {0}")]
    DecodeError(String),
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
    #[error("Protocol not found: {0}")]
    NotFound(String),
}

/// Core trait for protocol implementations.
///
/// Each protocol can:
/// - Encode: Transform data going outward (app -> network)
/// - Decode: Transform data coming inward (network -> app)
pub trait Protocol: Send + Sync {
    /// Unique name for this protocol
    fn name(&self) -> &'static str;

    /// Encode data going outward (toward network)
    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError>;

    /// Decode data coming inward (from network)
    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError>;

    /// Get protocol status/info
    fn status(&self) -> Value;
}

/// Configuration for a single protocol layer.
///
/// This allows protocols to be configured dynamically via JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "snake_case")]
pub enum ProtocolConfig {
    /// No-op pass-through protocol
    Identity,
    /// Base64 encoding
    Base64(Base64Config),
    /// HTTP protocol
    Http(HttpConfig),
    /// TCP raw protocol
    Tcp(TcpConfig),
    /// WebSocket protocol
    WebSocket(WebSocketConfig),
    /// Custom protocol (for future extensions)
    Custom { name: String, config: Value },
}

/// Base64 encoding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Base64Config {
    /// Use URL-safe base64 variant
    #[serde(default)]
    pub url_safe: bool,
    /// Add padding
    #[serde(default = "default_true")]
    pub padding: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Base64Config {
    fn default() -> Self {
        Self {
            url_safe: false,
            padding: true,
        }
    }
}

/// HTTP protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// HTTP method
    #[serde(default = "default_post")]
    pub method: String,
    /// Request path
    #[serde(default)]
    pub path: String,
    /// Headers to add
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// User agent
    #[serde(default)]
    pub user_agent: String,
}

fn default_post() -> String {
    "POST".to_string()
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            method: "POST".to_string(),
            path: "/".to_string(),
            headers: std::collections::HashMap::new(),
            user_agent: "TreeProtocol/1.0".to_string(),
        }
    }
}

/// TCP raw protocol configuration  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConfig {
    /// Delimiter for message framing
    #[serde(default)]
    pub delimiter: String,
    /// Include length prefix
    #[serde(default)]
    pub length_prefix: bool,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            delimiter: "\n".to_string(),
            length_prefix: false,
        }
    }
}

/// WebSocket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// WebSocket subprotocol
    #[serde(default)]
    pub subprotocol: Option<String>,
    /// Path for WS connection
    #[serde(default)]
    pub path: String,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            subprotocol: None,
            path: "/".to_string(),
        }
    }
}

/// A stack of protocols to process data through.
///
/// Data flows through the stack:
/// - Encoding: App -> Protocol N -> ... -> Protocol 1 -> Network
/// - Decoding: Network -> Protocol 1 -> ... -> Protocol N -> App
pub struct ProtocolStack {
    /// Stack of protocols (outermost first for encoding)
    protocols: Vec<Box<dyn Protocol>>,
    /// Configuration order (for serialization)
    config_order: Vec<String>,
}

impl Clone for ProtocolStack {
    fn clone(&self) -> Self {
        Self {
            protocols: Vec::new(), // Can't clone protocols
            config_order: self.config_order.clone(),
        }
    }
}

impl std::fmt::Debug for ProtocolStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolStack")
            .field("config_order", &self.config_order)
            .field("protocols", &self.protocols.len())
            .finish()
    }
}

impl ProtocolStack {
    /// Create a new empty stack
    pub fn new() -> Self {
        Self {
            protocols: Vec::new(),
            config_order: Vec::new(),
        }
    }

    /// Create a stack from configurations
    pub fn from_configs(configs: &[ProtocolConfig]) -> Result<Self, ProtocolError> {
        let mut stack = Self::new();
        for config in configs {
            stack.push(config)?;
        }
        Ok(stack)
    }

    /// Add a protocol to the stack (outermost position)
    pub fn push(&mut self, config: &ProtocolConfig) -> Result<(), ProtocolError> {
        let (protocol, name) = match config {
            ProtocolConfig::Identity => {
                let p = IdentityProtocol::new();
                (Box::new(p) as Box<dyn Protocol>, "identity".to_string())
            }
            ProtocolConfig::Base64(cfg) => {
                let p = Base64Protocol::new(cfg.clone());
                (Box::new(p) as Box<dyn Protocol>, "base64".to_string())
            }
            ProtocolConfig::Http(cfg) => {
                let p = HttpProtocol::new(cfg.clone());
                (Box::new(p) as Box<dyn Protocol>, "http".to_string())
            }
            ProtocolConfig::Tcp(cfg) => {
                let p = TcpProtocol::new(cfg.clone());
                (Box::new(p) as Box<dyn Protocol>, "tcp".to_string())
            }
            ProtocolConfig::WebSocket(cfg) => {
                let p = WebSocketProtocol::new(cfg.clone());
                (Box::new(p) as Box<dyn Protocol>, "websocket".to_string())
            }
            ProtocolConfig::Custom { name, config: _ } => {
                return Err(ProtocolError::NotFound(format!(
                    "Custom protocol '{}' not implemented",
                    name
                )));
            }
        };

        self.config_order.push(name);
        self.protocols.push(protocol);
        Ok(())
    }

    /// Remove the outermost protocol
    pub fn pop(&mut self) -> Option<Box<dyn Protocol>> {
        self.config_order.pop()?;
        self.protocols.pop()
    }

    /// Get number of protocols in stack
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// Encode data through the entire stack (app -> network)
    pub fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let mut result = data.to_vec();
        for protocol in self.protocols.iter() {
            result = protocol.encode(&result)?;
        }
        Ok(result)
    }

    /// Decode data through the entire stack (network -> app)
    pub fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let mut result = data.to_vec();
        // Decode in reverse order (innermost to outermost)
        for protocol in self.protocols.iter().rev() {
            result = protocol.decode(&result)?;
        }
        Ok(result)
    }

    /// Encode a TreeMessage through the stack
    pub fn encode_message(&self, message: &TreeMessage) -> Result<Vec<u8>, ProtocolError> {
        let json =
            serde_json::to_vec(message).map_err(|e| ProtocolError::EncodeError(e.to_string()))?;
        self.encode(&json)
    }

    /// Decode data into a TreeMessage
    pub fn decode_message(&self, data: &[u8]) -> Result<TreeMessage, ProtocolError> {
        let decoded = self.decode(data)?;
        serde_json::from_slice(&decoded).map_err(|e| ProtocolError::DecodeError(e.to_string()))
    }

    /// Get status of all protocols in stack
    pub fn status(&self) -> Vec<Value> {
        self.protocols.iter().map(|p| p.status()).collect()
    }

    /// Get the configuration for serialization
    pub fn to_configs(&self) -> Vec<ProtocolConfig> {
        self.config_order
            .iter()
            .enumerate()
            .filter_map(|(_, name)| {
                // This is simplified - in production you'd store configs
                Some(match name.as_str() {
                    "identity" => ProtocolConfig::Identity,
                    "base64" => ProtocolConfig::Base64(Default::default()),
                    "http" => ProtocolConfig::Http(Default::default()),
                    "tcp" => ProtocolConfig::Tcp(Default::default()),
                    "websocket" => ProtocolConfig::WebSocket(Default::default()),
                    _ => return None,
                })
            })
            .collect()
    }
}

impl Default for ProtocolStack {
    fn default() -> Self {
        Self::new()
    }
}

/// TCP protocol implementation (simple framing)
pub struct TcpProtocol {
    config: TcpConfig,
}

impl TcpProtocol {
    pub fn new(config: TcpConfig) -> Self {
        Self { config }
    }
}

impl Protocol for TcpProtocol {
    fn name(&self) -> &'static str {
        "tcp"
    }

    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let mut result = Vec::new();

        if self.config.length_prefix {
            let len = (data.len() as u32).to_be_bytes();
            result.extend_from_slice(&len);
        }

        result.extend_from_slice(data);

        if !self.config.length_prefix && !self.config.delimiter.is_empty() {
            result.extend_from_slice(self.config.delimiter.as_bytes());
        }

        Ok(result)
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let mut result = data.to_vec();

        // Remove delimiter if present
        if !self.config.delimiter.is_empty() {
            if let Some(pos) = result
                .iter()
                .position(|&b| self.config.delimiter.as_bytes().contains(&b))
            {
                result.truncate(pos);
            }
        }

        // If length prefix, skip it
        if self.config.length_prefix && result.len() >= 4 {
            let len = u32::from_be_bytes([result[0], result[1], result[2], result[3]]) as usize;
            if result.len() >= 4 + len {
                result = result[4..4 + len].to_vec();
            }
        }

        Ok(result)
    }

    fn status(&self) -> Value {
        serde_json::json!({
            "protocol": "tcp",
            "delimiter": self.config.delimiter,
            "length_prefix": self.config.length_prefix,
        })
    }
}

/// WebSocket protocol implementation (simplified)
pub struct WebSocketProtocol {
    config: WebSocketConfig,
}

impl WebSocketProtocol {
    pub fn new(config: WebSocketConfig) -> Self {
        Self { config }
    }
}

impl Protocol for WebSocketProtocol {
    fn name(&self) -> &'static str {
        "websocket"
    }

    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        // Simple WebSocket text frame: FIN(1) + opcode(1) + length(2) + data
        let mut frame = vec![0x81]; // FIN + text opcode
        let len = data.len();
        if len < 126 {
            frame.push(len as u8);
        } else if len < 65536 {
            frame.push(126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        frame.extend_from_slice(data);
        Ok(frame)
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if data.len() < 2 {
            return Err(ProtocolError::DecodeError("Frame too short".to_string()));
        }

        let opcode = data[0] & 0x0f;
        if opcode == 0x08 {
            // Close frame
            return Err(ProtocolError::DecodeError("Connection closed".to_string()));
        }

        let len = data[1] & 0x7f;
        let header_len = match len {
            126 => 4,
            127 => 10,
            _ => 2,
        };

        if data.len() > header_len {
            Ok(data[header_len..].to_vec())
        } else {
            Err(ProtocolError::DecodeError("Incomplete frame".to_string()))
        }
    }

    fn status(&self) -> Value {
        serde_json::json!({
            "protocol": "websocket",
            "path": self.config.path,
            "subprotocol": self.config.subprotocol,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_stack() {
        let mut stack = ProtocolStack::new();
        stack
            .push(&ProtocolConfig::Base64(Default::default()))
            .unwrap();

        let data = b"hello world";
        let encoded = stack.encode(data).unwrap();
        let decoded = stack.decode(&encoded).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_multi_layer_stack() {
        let mut stack = ProtocolStack::new();
        stack
            .push(&ProtocolConfig::Base64(Default::default()))
            .unwrap();
        stack
            .push(&ProtocolConfig::Tcp(Default::default()))
            .unwrap();

        let data = b"test message";
        let encoded = stack.encode(data).unwrap();
        let decoded = stack.decode(&encoded).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_http_config() {
        let config = HttpConfig {
            method: "POST".to_string(),
            path: "/api/test".to_string(),
            headers: std::collections::HashMap::new(),
            user_agent: "Test/1.0".to_string(),
        };

        let mut stack = ProtocolStack::new();
        stack.push(&ProtocolConfig::Http(config)).unwrap();

        assert_eq!(stack.len(), 1);
    }
}
