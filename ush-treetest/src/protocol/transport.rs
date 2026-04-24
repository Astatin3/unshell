//! # Transport Layer
//!
//! This module provides the Transport trait and TCP implementation.
//! Uses a simple length-prefixed framing: `[u32 header_len][header bytes][u32 payload_len][payload bytes]`
//!
//! # Frame Format
//!
//! Each frame is encoded as:
//! - 4 bytes: header length (little-endian u32)
//! - N bytes: serialized header
//! - 4 bytes: payload length (little-endian u32)
//! - M bytes: payload (optional)
//!
//! # Usage
//!
//! ```no_run
//! use ush_treetest::protocol::{TcpTransport, Transport, FrameHeader, FrameType};
//!
//! // Connect to server
//! let mut transport = TcpTransport::connect("localhost:8080").unwrap();
//!
//! // Send a frame
//! let header = FrameHeader {
//!     frame_type: FrameType::Request,
//!     dst_path: Some("/shell".to_string()),
//!     src_path: "/client".to_string(),
//!     request_id: Some(1),
//!     stream_id: None,
//! };
//! transport.send_frame(&header, Some(b"test payload")).unwrap();
//!
//! // Receive a frame
//! let (header, payload) = transport.recv_frame().unwrap();
//! ```

use crate::protocol::types::*;
use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write, Error};

/// Transport trait - interface for sending and receiving frames.
///
/// This trait defines the interface for all transport implementations.
/// Implementors must provide send_frame, recv_frame, and close methods.
pub trait Transport: Sized {
    /// Error type for this transport
    type Error: std::fmt::Debug;

    /// Send a frame (header + optional payload).
    ///
    /// # Arguments
    /// * `header` - The frame header
    /// * `payload` - Optional payload bytes
    fn send_frame(
        &mut self,
        header: &FrameHeader,
        payload: Option<&[u8]>,
    ) -> Result<(), Self::Error>;

    /// Receive a frame.
    ///
    /// # Returns
    /// (header, payload) tuple
    fn recv_frame(&mut self) -> Result<(FrameHeader, Vec<u8>), Self::Error>;

    /// Close the transport.
    #[allow(dead_code)]
    fn close(&mut self) -> Result<(), Self::Error>;
}

/// Transport-level errors.
///
/// # Variants
/// * `ConnectionClosed` - The connection was closed
/// * `InvalidFrame` - The frame was invalid
/// * `Io` - I/O error
#[derive(Debug)]
pub enum TransportError {
    /// Connection was closed
    ConnectionClosed,
    /// Invalid frame format
    InvalidFrame(String),
    /// I/O error
    Io(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::ConnectionClosed => write!(f, "connection closed"),
            TransportError::InvalidFrame(s) => write!(f, "invalid frame: {}", s),
            TransportError::Io(s) => write!(f, "I/O error: {}", s),
        }
    }
}

impl From<Error> for TransportError {
    fn from(e: Error) -> Self {
        TransportError::Io(e.to_string())
    }
}

/// TCP transport implementation.
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Create a new TCP transport from an existing stream.
    ///
    /// Sets read/write timeouts to 30 seconds for safety.
    ///
    /// # Arguments
    /// * `stream` - An existing TCP stream
    pub fn new(stream: TcpStream) -> Self {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .ok();
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(30)))
            .ok();
        Self { stream }
    }

    /// Connect to a remote address.
    ///
    /// # Arguments
    /// * `addr` - The address to connect to (e.g., "localhost:8080")
    ///
    /// # Returns
    /// Connected transport
    ///
    /// # Example
    /// ```
    /// let transport = TcpTransport::connect("localhost:8080").unwrap();
    /// ```
    pub fn connect(addr: &str) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self::new(stream))
    }

    /// Create a listening socket.
    ///
    /// # Arguments
    /// * `addr` - The address to listen on
    ///
    /// # Returns
    /// TCP listener
    ///
    /// # Example
    /// ```
    /// let listener = TcpTransport::listen("0.0.0.0:8080").unwrap();
    /// ```
    pub fn listen(addr: &str) -> Result<std::net::TcpListener, TransportError> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(false)?;
        Ok(listener)
    }

    /// Accept an incoming connection.
    ///
    /// # Arguments
    /// * `listener` - The listening socket
    ///
    /// # Returns
    /// New transport for the connection
    ///
    /// # Example
    /// ```
    /// let listener = TcpTransport::listen("0.0.0.0:8080").unwrap();
    /// let transport = TcpTransport::accept(&listener).unwrap();
    /// ```
    pub fn accept(listener: &std::net::TcpListener) -> Result<Self, TransportError> {
        let stream = listener.accept()?.0;
        Ok(Self::new(stream))
    }

    /// Get peer address.
    ///
    /// # Returns
    /// The peer's socket address
    pub fn peer_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.stream.peer_addr()
    }

    /// Read exactly n bytes.
    ///
    /// Will block until all bytes are read or an error occurs.
    fn read_exact(&mut self, mut n: usize) -> Result<Vec<u8>, TransportError> {
        let mut buf = Vec::with_capacity(n);
        while n > 0 {
            let mut chunk = vec![0u8; n];
            let read =
                self.stream
                    .read(&mut chunk)
                    .map_err(|e| TransportError::Io(e.to_string()))?;
            if read == 0 {
                return Err(TransportError::ConnectionClosed);
            }
            buf.extend_from_slice(&chunk[..read]);
            n -= read;
        }
        Ok(buf)
    }
}

impl Transport for TcpTransport {
    type Error = TransportError;

    fn send_frame(
        &mut self,
        header: &FrameHeader,
        payload: Option<&[u8]>,
    ) -> Result<(), Self::Error> {
        let header_bytes = header.to_bytes();
        let header_len = header_bytes.len() as u32;

        let payload_bytes = payload.unwrap_or(&[]);
        let payload_len = payload_bytes.len() as u32;

        let mut frame =
            Vec::with_capacity(4 + header_len as usize + 4 + payload_len as usize);
        frame.extend_from_slice(&header_len.to_le_bytes());
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&payload_len.to_le_bytes());
        frame.extend_from_slice(payload_bytes);

        self.stream
            .write_all(&frame)
            .map_err(|e| TransportError::Io(e.to_string()))?;
        self.stream
            .flush()
            .map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(())
    }

    fn recv_frame(&mut self) -> Result<(FrameHeader, Vec<u8>), Self::Error> {
        let header_len_bytes = self.read_exact(4)?;
        let header_len = u32::from_le_bytes(header_len_bytes.try_into().unwrap()) as usize;

        let header_bytes = self.read_exact(header_len)?;
        let header =
            FrameHeader::from_bytes(&header_bytes).map_err(TransportError::InvalidFrame)?;

        let payload_len_bytes = self.read_exact(4)?;
        let payload_len =
            u32::from_le_bytes(payload_len_bytes.try_into().unwrap()) as usize;

        let payload = if payload_len > 0 {
            self.read_exact(payload_len)?
        } else {
            Vec::new()
        };

        Ok((header, payload))
    }

    fn close(&mut self) -> Result<(), Self::Error> {
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(())
    }
}

/// Create a request frame.
///
/// # Arguments
/// * `dst_path` - Destination path
/// * `src_path` - Source path
/// * `request_id` - Request ID
/// * `request` - The request payload
///
/// # Returns
/// (header, payload) tuple
///
/// # Example
/// ```
/// use ush_treetest::protocol::{make_request, TreeRequest};
///
/// let request = TreeRequest::Exec { cmd: "echo hello".to_string() };
/// let (header, payload) = make_request("/shell", "/client", 1, &request);
/// ```
pub fn make_request(
    dst_path: &str,
    src_path: &str,
    request_id: u64,
    request: &TreeRequest,
) -> (FrameHeader, Vec<u8>) {
    let header = FrameHeader {
        frame_type: FrameType::Request,
        dst_path: Some(dst_path.to_string()),
        src_path: src_path.to_string(),
        request_id: Some(request_id),
        stream_id: None,
    };
    let payload = request.to_bytes();
    (header, payload)
}

/// Create a response frame.
///
/// # Arguments
/// * `src_path` - Source path
/// * `request_id` - Request ID
/// * `response` - The response payload
///
/// # Returns
/// (header, payload) tuple
pub fn make_response(
    src_path: &str,
    request_id: u64,
    response: &TreeResponse,
) -> (FrameHeader, Vec<u8>) {
    let header = FrameHeader {
        frame_type: FrameType::Response,
        dst_path: None,
        src_path: src_path.to_string(),
        request_id: Some(request_id),
        stream_id: None,
    };
    let payload = response.to_bytes();
    (header, payload)
}

/// Create a stream open frame.
///
/// # Arguments
/// * `dst_path` - Destination path
/// * `src_path` - Source path
/// * `request_id` - Request ID
///
/// # Returns
/// Frame header (no payload)
pub fn make_stream_open(dst_path: &str, src_path: &str, request_id: u64) -> FrameHeader {
    FrameHeader {
        frame_type: FrameType::StreamOpen,
        dst_path: Some(dst_path.to_string()),
        src_path: src_path.to_string(),
        request_id: Some(request_id),
        stream_id: None,
    }
}

/// Create a stream data frame.
///
/// # Arguments
/// * `stream_id` - Stream ID
/// * `data` - Data to send
///
/// # Returns
/// (header, payload) tuple
pub fn make_stream_data(stream_id: u16, data: &[u8]) -> (FrameHeader, Vec<u8>) {
    let header = FrameHeader {
        frame_type: FrameType::StreamData,
        dst_path: None,
        src_path: String::new(),
        request_id: None,
        stream_id: Some(stream_id),
    };
    (header, data.to_vec())
}

/// Create a stream close frame.
///
/// # Arguments
/// * `stream_id` - Stream ID to close
///
/// # Returns
/// Frame header (no payload)
pub fn make_stream_close(stream_id: u16) -> FrameHeader {
    FrameHeader {
        frame_type: FrameType::StreamClose,
        dst_path: None,
        src_path: String::new(),
        request_id: None,
        stream_id: Some(stream_id),
    }
}

/// Create a handshake frame.
///
/// # Arguments
/// * `registered_paths` - Paths to register
///
/// # Returns
/// (header, payload) tuple
///
/// # Example
/// ```
/// use ush_treetest::protocol::make_handshake;
///
/// let paths = vec!["/client".to_string()];
/// let (header, payload) = make_handshake(paths);
/// ```
pub fn make_handshake(registered_paths: Vec<String>) -> (FrameHeader, Vec<u8>) {
    let handshake = Handshake {
        registered_paths,
    };
    let payload = handshake.to_bytes();
    let header = FrameHeader {
        frame_type: FrameType::Handshake,
        dst_path: None,
        src_path: String::new(),
        request_id: None,
        stream_id: None,
    };
    (header, payload)
}

/// Create a handshake ack frame.
///
/// # Arguments
/// * `accepted` - Whether handshake was accepted
/// * `assigned_base_path` - Base path to assign
///
/// # Returns
/// (header, payload) tuple
pub fn make_handshake_ack(
    accepted: bool,
    assigned_base_path: &str,
) -> (FrameHeader, Vec<u8>) {
    let ack = HandshakeAck {
        accepted,
        assigned_base_path: assigned_base_path.to_string(),
    };
    let payload = ack.to_bytes();
    let header = FrameHeader {
        frame_type: FrameType::HandshakeAck,
        dst_path: None,
        src_path: String::new(),
        request_id: None,
        stream_id: None,
    };
    (header, payload)
}