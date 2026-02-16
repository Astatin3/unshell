//! HTTP protocol implementation for tree communication.
//!
//! This protocol wraps data in HTTP requests/responses for traffic blending.

use crate::tree::protocols::stack::{HttpConfig, Protocol, ProtocolError};
use serde_json::Value;

/// HTTP protocol for tree communication.
///
/// Wraps outgoing data in HTTP requests and parses incoming HTTP responses.
pub struct HttpProtocol {
    config: HttpConfig,
}

impl HttpProtocol {
    pub fn new(config: HttpConfig) -> Self {
        Self { config }
    }

    /// Build HTTP request
    fn build_request(&self, body: &[u8]) -> Vec<u8> {
        let body_len = body.len();
        let body_str = String::from_utf8_lossy(body);

        let mut request = format!(
            "{} {} HTTP/1.1\r\n\
             Host: {}\r\n\
             User-Agent: {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n",
            self.config.method,
            self.config.path,
            "localhost", // Would be configured in production
            self.config.user_agent,
            body_len
        );

        // Add custom headers
        for (key, value) in &self.config.headers {
            request.push_str(&format!("{}: {}\r\n", key, value));
        }

        request.push_str("\r\n");
        request.push_str(&body_str);

        request.into_bytes()
    }

    /// Parse HTTP response and extract body
    fn parse_response(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let data_str = String::from_utf8(data.to_vec())
            .map_err(|e| ProtocolError::DecodeError(e.to_string()))?;

        // Find body start (after \r\n\r\n)
        let body_start = match data_str.find("\r\n\r\n") {
            Some(pos) => pos + 4,
            None => {
                return Err(ProtocolError::DecodeError(
                    "Invalid HTTP response".to_string(),
                ))
            }
        };

        // Extract status code
        let status_line = data_str
            .split("\r\n")
            .next()
            .ok_or_else(|| ProtocolError::DecodeError("No status line".to_string()))?;

        let status_code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| ProtocolError::DecodeError("Invalid status code".to_string()))?;

        if status_code < 200 || status_code >= 300 {
            return Err(ProtocolError::DecodeError(format!(
                "HTTP error: {}",
                status_code
            )));
        }

        // Extract body
        let body = &data_str[body_start..];
        Ok(body.as_bytes().to_vec())
    }
}

impl Protocol for HttpProtocol {
    fn name(&self) -> &'static str {
        "http"
    }

    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        Ok(self.build_request(data))
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        self.parse_response(data)
    }

    fn status(&self) -> Value {
        serde_json::json!({
            "protocol": "http",
            "method": self.config.method,
            "path": self.config.path,
            "headers": self.config.headers,
            "user_agent": self.config.user_agent,
        })
    }
}

/// HTTP server for receiving tree messages.
///
/// This is a simple implementation for testing - in production you'd
/// use a proper HTTP server.
pub struct HttpServer {
    config: HttpConfig,
}

impl HttpServer {
    pub fn new(config: HttpConfig) -> Self {
        Self { config }
    }

    /// Parse incoming HTTP request and extract body
    pub fn parse_request(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let data_str = String::from_utf8(data.to_vec())
            .map_err(|e| ProtocolError::DecodeError(e.to_string()))?;

        // Find body start
        let body_start = match data_str.find("\r\n\r\n") {
            Some(pos) => pos + 4,
            None => {
                return Err(ProtocolError::DecodeError(
                    "Invalid HTTP request".to_string(),
                ))
            }
        };

        // Extract Content-Length
        let mut content_length = 0;
        for line in data_str.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                content_length = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
                break;
            }
        }

        let body = &data_str[body_start..];
        if body.len() >= content_length {
            Ok(body[..content_length].as_bytes().to_vec())
        } else {
            Ok(body.as_bytes().to_vec())
        }
    }

    /// Build HTTP response
    pub fn build_response(&self, body: &[u8], status: u16) -> Vec<u8> {
        let body_len = body.len();
        let body_str = String::from_utf8_lossy(body);

        let status_text = match status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        };

        format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            status, status_text, body_len, body_str
        )
        .into_bytes()
    }
}

impl Default for HttpServer {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_encode_decode() {
        let proto = HttpProtocol::new(Default::default());

        let data = r#"{"action": "test", "data": "hello"}"#.as_bytes();
        let encoded = proto.encode(data).unwrap();

        // Build a valid response
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 29\r\n\r\n{\"action\": \"test\", \"data\": \"hello\"}";

        let decoded = proto.decode(response).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_request_building() {
        let server = HttpServer::new(Default::default());

        let body = r#"{"test": "data"}"#.as_bytes();
        let request = server.build_request(body);

        let request_str = String::from_utf8(request).unwrap();
        assert!(request_str.contains("POST / HTTP/1.1"));
        assert!(request_str.contains("Content-Length: 16"));
    }
}
