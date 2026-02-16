//! TreeMessage - Serializable message format for network communication.
//!
//! This module defines the message structure used for all tree communications.
//! The format is designed to be simple, extensible, and protocol-agnostic.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Message types for tree communication
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

/// Core message structure for all tree communications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeMessage {
    /// Unique identifier for message correlation
    pub id: String,
    /// Type of message (request, response, event, stream)
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    /// Target path in the tree (for routing)
    #[serde(default)]
    pub target: Vec<String>,
    /// Action to perform (Get, Set, Invoke, etc.)
    #[serde(default)]
    pub action: String,
    /// Payload/data for the action
    #[serde(default)]
    pub payload: Value,
    /// ID of the message this is a response to
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_to: Option<String>,
    /// Stream ID for streaming communications
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
}

impl TreeMessage {
    /// Create a new request message
    pub fn new_req(id: impl Into<String>, target: Vec<String>, action: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Req,
            target,
            action: action.into(),
            payload: Value::Null,
            response_to: None,
            stream_id: None,
        }
    }

    /// Create a response message
    pub fn new_resp(id: impl Into<String>, response_to: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Resp,
            target: vec![],
            action: String::new(),
            payload,
            response_to: Some(response_to.into()),
            stream_id: None,
        }
    }

    /// Create an event message
    pub fn new_event(id: impl Into<String>, target: Vec<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Event,
            target,
            action: String::new(),
            payload,
            response_to: None,
            stream_id: None,
        }
    }

    /// Create a stream message
    pub fn new_stream(id: impl Into<String>, stream_id: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Stream,
            target: vec![],
            action: String::new(),
            payload,
            response_to: None,
            stream_id: Some(stream_id.into()),
        }
    }
}

impl Default for TreeMessage {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            msg_type: MessageType::Req,
            target: vec![],
            action: String::new(),
            payload: Value::Null,
            response_to: None,
            stream_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = TreeMessage::new_req("test-1", vec!["a".to_string(), "b".to_string()], "Get");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"req\""));
        assert!(json.contains("\"target\":[\"a\",\"b\"]"));
        assert!(json.contains("\"action\":\"Get\""));
    }

    #[test]
    fn test_response_message() {
        let msg = TreeMessage::new_resp("resp-1", "test-1", json!("value"));
        assert_eq!(msg.msg_type, MessageType::Resp);
        assert_eq!(msg.response_to, Some("test-1".to_string()));
    }
}
