//! # Transport Layer
//!
//! This module provides the Transport trait and TCP implementation.
//! Uses a simple length-prefixed framing: [u32 header_len][header bytes][u32 payload_len][payload bytes]

use crate::protocol::types::*;
use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write, Error};

pub trait Transport: Sized {
    type Error: std::fmt::Debug;
    /// Send a frame (header + optional payload)
    fn send_frame(&mut self, header: &FrameHeader, payload: Option<&[u8]>) -> Result<(), Self::Error>;
    /// Receive a frame
    fn recv_frame(&mut self) -> Result<(FrameHeader, Vec<u8>), Self::Error>;
    /// Close the transport
    #[allow(dead_code)]
    fn close(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum TransportError {
    ConnectionClosed,
    InvalidFrame(String),
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
    fn from(e: Error) -> Self { TransportError::Io(e.to_string()) }
}

/// TCP transport implementation
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        // Set timeouts for safety
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30))).ok();
        stream.set_write_timeout(Some(std::time::Duration::from_secs(30))).ok();
        Self { stream }
    }
    
    /// Connect to a remote address
    pub fn connect(addr: &str) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self::new(stream))
    }
    
    /// Create a listening socket
    pub fn listen(addr: &str) -> Result<std::net::TcpListener, TransportError> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(false)?;
        Ok(listener)
    }
    
    /// Accept an incoming connection
    pub fn accept(listener: &std::net::TcpListener) -> Result<Self, TransportError> {
        let stream = listener.accept()?.0;
        Ok(Self::new(stream))
    }
    
    /// Get peer address
    pub fn peer_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.stream.peer_addr()
    }
    
    /// Read exactly n bytes
    fn read_exact(&mut self, mut n: usize) -> Result<Vec<u8>, TransportError> {
        let mut buf = Vec::with_capacity(n);
        while n > 0 {
            let mut chunk = vec![0u8; n];
            let read = self.stream.read(&mut chunk).map_err(|e| TransportError::Io(e.to_string()))?;
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
    
    fn send_frame(&mut self, header: &FrameHeader, payload: Option<&[u8]>) -> Result<(), Self::Error> {
        // Serialize header using rkyv
        let header_bytes = header.to_bytes();
        let header_len = header_bytes.len() as u32;
        
        // Get payload bytes
        let payload_bytes = payload.unwrap_or(&[]);
        let payload_len = payload_bytes.len() as u32;
        
        // Build frame: [u32 header_len][header][u32 payload_len][payload]
        let mut frame = Vec::with_capacity(4 + header_len as usize + 4 + payload_len as usize);
        frame.extend_from_slice(&header_len.to_le_bytes());
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&payload_len.to_le_bytes());
        frame.extend_from_slice(payload_bytes);
        
        self.stream.write_all(&frame).map_err(|e| TransportError::Io(e.to_string()))?;
        self.stream.flush().map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(())
    }
    
    fn recv_frame(&mut self) -> Result<(FrameHeader, Vec<u8>), Self::Error> {
        // Read header length
        let header_len_bytes = self.read_exact(4)?;
        let header_len = u32::from_le_bytes(header_len_bytes.try_into().unwrap()) as usize;
        
        // Read header
        let header_bytes = self.read_exact(header_len)?;
        let header = FrameHeader::from_bytes(&header_bytes).map_err(|e| TransportError::InvalidFrame(e))?;
        
        // Read payload length
        let payload_len_bytes = self.read_exact(4)?;
        let payload_len = u32::from_le_bytes(payload_len_bytes.try_into().unwrap()) as usize;
        
        // Read payload
        let payload = if payload_len > 0 {
            self.read_exact(payload_len)?
        } else {
            Vec::new()
        };
        
        Ok((header, payload))
    }
    
    fn close(&mut self) -> Result<(), Self::Error> {
        self.stream.shutdown(std::net::Shutdown::Both).map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(())
    }
}

// =============================================================================
// Frame builder functions
// =============================================================================

/// Create a request frame
pub fn make_request(dst_path: &str, src_path: &str, request_id: u64, request: &TreeRequest) -> (FrameHeader, Vec<u8>) {
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

/// Create a response frame  
pub fn make_response(src_path: &str, request_id: u64, response: &TreeResponse) -> (FrameHeader, Vec<u8>) {
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

/// Create a stream open frame
pub fn make_stream_open(dst_path: &str, src_path: &str, request_id: u64) -> FrameHeader {
    FrameHeader {
        frame_type: FrameType::StreamOpen,
        dst_path: Some(dst_path.to_string()),
        src_path: src_path.to_string(),
        request_id: Some(request_id),
        stream_id: None,
    }
}

/// Create a stream data frame
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

/// Create a stream close frame
pub fn make_stream_close(stream_id: u16) -> FrameHeader {
    FrameHeader {
        frame_type: FrameType::StreamClose,
        dst_path: None,
        src_path: String::new(),
        request_id: None,
        stream_id: Some(stream_id),
    }
}

/// Create a handshake frame
pub fn make_handshake(registered_paths: Vec<String>) -> (FrameHeader, Vec<u8>) {
    let handshake = Handshake { registered_paths };
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

/// Create a handshake ack frame
pub fn make_handshake_ack(accepted: bool, assigned_base_path: &str) -> (FrameHeader, Vec<u8>) {
    let ack = HandshakeAck { 
        accepted, 
        assigned_base_path: assigned_base_path.to_string() 
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