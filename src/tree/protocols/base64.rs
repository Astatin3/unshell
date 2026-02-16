//! Base64 encoding/decoding protocol.

use crate::tree::protocols::stack::{Base64Config, Protocol, ProtocolError};
use serde_json::Value;

/// Base64 encoding protocol
pub struct Base64Protocol {
    config: Base64Config,
}

impl Base64Protocol {
    pub fn new(config: Base64Config) -> Self {
        Self { config }
    }
}

impl Protocol for Base64Protocol {
    fn name(&self) -> &'static str {
        "base64"
    }

    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let encoded = if self.config.url_safe {
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, data)
        } else if self.config.padding {
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
        } else {
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, data)
        };

        Ok(encoded.into_bytes())
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let data_str = String::from_utf8(data.to_vec())
            .map_err(|e| ProtocolError::DecodeError(e.to_string()))?;

        let decoded = if self.config.url_safe {
            base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &data_str)
        } else if self.config.padding {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data_str)
        } else {
            // Try standard first, then URL-safe
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data_str).or_else(
                |_| {
                    base64::Engine::decode(
                        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                        &data_str,
                    )
                },
            )
        };

        decoded.map_err(|e| ProtocolError::DecodeError(e.to_string()))
    }

    fn status(&self) -> Value {
        serde_json::json!({
            "protocol": "base64",
            "url_safe": self.config.url_safe,
            "padding": self.config.padding,
        })
    }
}

/// Identity (pass-through) protocol
pub struct IdentityProtocol;

impl IdentityProtocol {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IdentityProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl Protocol for IdentityProtocol {
    fn name(&self) -> &'static str {
        "identity"
    }

    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        Ok(data.to_vec())
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        Ok(data.to_vec())
    }

    fn status(&self) -> Value {
        serde_json::json!({
            "protocol": "identity",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        let proto = Base64Protocol::new(Default::default());
        let data = b"Hello, World!";
        let encoded = proto.encode(data).unwrap();
        let decoded = proto.decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_base64_url_safe() {
        let proto = Base64Protocol::new(Base64Config {
            url_safe: true,
            padding: false,
        });
        let data = b"test+/data";
        let encoded = proto.encode(data).unwrap();
        let encoded_str = String::from_utf8(encoded).unwrap();
        assert!(!encoded_str.contains('+'));
        assert!(!encoded_str.contains('/'));
    }
}
