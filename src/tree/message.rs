//! TreeMessage - Serializable message format for network communication.
//!
//! This module defines the message structure used for all tree communications.
//! The format is designed to be simple, extensible, and protocol-agnostic.
//! Based on SPEC.md - supports RPC, streams, events, and P2P pivoting.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Message types for transport-level distinction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    /// Request message - expecting a response
    Req,
    /// Response message - reply to a request
    Resp,
    /// Event message - unsolicited notification
    Event,
    /// Stream message - bidirectional data flow
    Stream,
}

impl Default for MessageType {
    fn default() -> Self {
        Self::Req
    }
}

/// Core message structure for all tree communications.
///
/// This structure follows SPEC.md with loose typing for extensibility.
/// All fields are optional except where noted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeMessage {
    /// Unique identifier for message correlation
    /// Used to match requests with responses
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Origin path for routing responses
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Value>,

    /// Destination path for routing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Value>,

    /// Operation to perform (e.g., "rpc.call", "stream.data", "event")
    /// This is the primary field - must be present
    pub action: Value,

    /// Data for the action - interpretation depends on action type
    #[serde(default)]
    pub payload: Value,

    /// P2P/pivoting metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<Value>,

    /// Extensible metadata (timing, transport hints, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,

    /// Type wrapper for transport-level distinction
    #[serde(default, rename = "type")]
    pub msg_type: MessageType,

    /// ID of the message this is a response to
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_to: Option<String>,

    /// Stream ID for streaming communications
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

impl TreeMessage {
    /// Create a new request message with minimal fields
    pub fn new(action: impl Into<Value>) -> Self {
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            source: None,
            target: None,
            action: action.into(),
            payload: Value::Null,
            routing: None,
            meta: None,
            msg_type: MessageType::Req,
            response_to: None,
            stream_id: None,
        }
    }

    /// Create a new request with target path
    pub fn to_target(mut self, target: impl Into<Value>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Create a new request with source path
    pub fn from_source(mut self, source: impl Into<Value>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Create a new request with payload
    pub fn with_payload(mut self, payload: impl Into<Value>) -> Self {
        self.payload = payload.into();
        self
    }

    /// Create a new request with routing info
    pub fn with_routing(mut self, routing: impl Into<Value>) -> Self {
        self.routing = Some(routing.into());
        self
    }

    /// Create a new request with metadata
    pub fn with_meta(mut self, meta: impl Into<Value>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Create a response message
    pub fn response(mut self, response_to: impl Into<String>) -> Self {
        self.msg_type = MessageType::Resp;
        self.response_to = Some(response_to.into());
        self.id = Some(uuid::Uuid::new_v4().to_string());
        self
    }

    /// Create an event message
    pub fn event(mut self) -> Self {
        self.msg_type = MessageType::Event;
        self
    }

    /// Create a stream message
    pub fn stream(mut self, stream_id: impl Into<String>) -> Self {
        self.msg_type = MessageType::Stream;
        self.stream_id = Some(stream_id.into());
        self
    }

    /// Check if action matches a pattern (simple string or namespaced)
    pub fn action_is(&self, action: &str) -> bool {
        match &self.action {
            Value::String(s) => s == action || s.ends_with(&format!(".{}", action)),
            _ => false,
        }
    }

    /// Get method name from RPC payload
    pub fn get_method(&self) -> Option<String> {
        self.payload
            .get("method")
            .and_then(|m| m.as_str())
            .map(String::from)
    }

    /// Get stream channel from payload
    pub fn get_channel(&self) -> Option<String> {
        self.payload
            .get("channel")
            .and_then(|c| c.as_str())
            .map(String::from)
    }

    /// Get target as a path vector
    pub fn get_target_path(&self) -> Option<Vec<String>> {
        self.target.as_ref().and_then(|t| match t {
            Value::String(s) => Some(s.split('/').map(String::from).collect()),
            Value::Array(arr) => {
                let mut path = Vec::new();
                for item in arr {
                    if let Some(s) = item.as_str() {
                        path.push(s.to_string());
                    }
                }
                if path.len() == arr.len() {
                    Some(path)
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    /// Get source as a path vector
    pub fn get_source_path(&self) -> Option<Vec<String>> {
        self.source.as_ref().and_then(|s| match s {
            Value::String(st) => Some(st.split('/').map(String::from).collect()),
            Value::Array(arr) => {
                let mut path = Vec::new();
                for item in arr {
                    if let Some(st) = item.as_str() {
                        path.push(st.to_string());
                    }
                }
                if path.len() == arr.len() {
                    Some(path)
                } else {
                    None
                }
            }
            _ => None,
        })
    }
}

impl Default for TreeMessage {
    fn default() -> Self {
        Self::new("query")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = TreeMessage::new("query")
            .to_target(vec!["a", "b"])
            .with_payload(json!({"key": "value"}));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"action\":\"query\""));
        assert!(json.contains("\"target\":[\"a\",\"b\"]"));
    }

    #[test]
    fn test_action_matching() {
        let msg = TreeMessage::new("rpc.call");
        assert!(msg.action_is("rpc.call"));
        assert!(msg.action_is("call"));
        assert!(!msg.action_is("stream.data"));
    }

    #[test]
    fn test_rpc_payload() {
        let msg = TreeMessage::new("rpc.call")
            .to_target(["components", "tcp-client"])
            .with_payload(json!({
                "method": "connect",
                "params": {"address": "127.0.0.1", "port": 443}
            }));

        assert_eq!(msg.get_method(), Some("connect".to_string()));
    }

    #[test]
    fn test_response() {
        let msg = TreeMessage::new("response").with_payload(json!({"success": true, "result": {}}));

        assert_eq!(msg.msg_type, MessageType::Resp);
    }
}
