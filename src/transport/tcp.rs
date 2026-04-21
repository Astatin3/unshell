//! # TCP Transport
//!
//! Only available when the `tcp` feature is enabled (requires `std`).
//! This file is only included in the module tree when `cfg(feature = "tcp")`,
//! as declared in `transport/mod.rs`.
//!
//! [`TcpTransport`] implements [`Transport`](super::Transport) over a
//! `std::net::TcpStream`.
//!
//! ## Framing
//!
//! Each `send` call writes:
//!
//! ```text
//! [u32 big-endian header_len]  [header bytes]
//! [u32 big-endian payload_len] [payload bytes]
//! ```
//!
//! Each `recv` call:
//! 1. Reads exactly 4 bytes → `header_len`.
//! 2. Checks `header_len <= MAX_HEADER_BYTES`.
//! 3. Reads exactly `header_len` bytes.
//! 4. Deserialises the `PacketHeader`.
//! 5. Reads exactly 4 bytes → `payload_len`.
//! 6. Checks `payload_len <= MAX_PAYLOAD_BYTES`.
//! 7. Reads exactly `payload_len` bytes.
//! 8. Returns `(header, payload)`.
//!
//! **All reads use `read_exact`.** TCP is a stream protocol; a single `read`
//! may return fewer bytes than requested. `read_exact` loops until it has
//! the full count or the stream ends.
//!
//! ## Reconnection
//!
//! `TcpTransport` does not handle reconnection internally. The caller (the
//! payload's main loop or the operator CLI) is responsible for catching
//! [`TransportError::Disconnected`] and [`TransportError::Io`], then
//! creating a new `TcpTransport` to the router address.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};

use super::{
    decode_header, encode_header, TransportError, Transport, MAX_HEADER_BYTES, MAX_PAYLOAD_BYTES,
};
use crate::protocol::PacketHeader;

/// A framed TCP transport wrapping a `TcpStream`.
///
/// # Example: connecting as a payload
///
/// ```rust,no_run
/// use unshell::transport::tcp::TcpTransport;
///
/// // Connect to the router
/// let transport = TcpTransport::connect("127.0.0.1:9000").expect("connection failed");
/// ```
///
/// # Example: accepting a connection on the router
///
/// ```rust,no_run
/// use std::net::TcpListener;
/// use unshell::transport::tcp::TcpTransport;
///
/// let listener = TcpListener::bind("0.0.0.0:9000").unwrap();
/// for stream in listener.incoming() {
///     let transport = TcpTransport::from_stream(stream.unwrap());
///     // hand off to a node thread
/// }
/// ```
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Connect to a remote address and return a transport wrapping that connection.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the connection fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unshell::transport::tcp::TcpTransport;
    /// let t = TcpTransport::connect("127.0.0.1:9000").unwrap();
    /// ```
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self { stream })
    }

    /// Wrap an already-connected `TcpStream`.
    ///
    /// Used by the router's accept loop, which creates streams via
    /// `TcpListener::incoming()`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::net::TcpListener;
    /// use unshell::transport::tcp::TcpTransport;
    ///
    /// let listener = TcpListener::bind("0.0.0.0:9000").unwrap();
    /// let (stream, _addr) = listener.accept().unwrap();
    /// let transport = TcpTransport::from_stream(stream);
    /// ```
    pub fn from_stream(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Access the underlying `TcpStream` for configuration (e.g., timeouts).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unshell::transport::tcp::TcpTransport;
    /// use std::time::Duration;
    ///
    /// let t = TcpTransport::connect("127.0.0.1:9000").unwrap();
    /// t.stream_ref().set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    /// ```
    pub fn stream_ref(&self) -> &TcpStream {
        &self.stream
    }
}

impl Transport for TcpTransport {
    /// Send a packet (header + payload) over the TCP stream.
    ///
    /// Writes the two-part frame atomically from the caller's perspective:
    /// this call does not return until all bytes have been written or an
    /// error occurs.
    ///
    /// # Errors
    ///
    /// - [`TransportError::Io`] on write failure or partial write.
    /// - [`TransportError::Disconnected`] if the remote closed the connection.
    fn send(&mut self, header: &PacketHeader, payload: &[u8]) -> Result<(), TransportError> {
        // Serialise the header
        let header_bytes =
            encode_header(header).ok_or(TransportError::DeserialiseError)?;

        // Build the full frame in one allocation so we can use a single
        // write_all() call, reducing the chance of partial writes causing
        // the remote to see a split frame.
        //
        // Frame layout:
        //   [u32 header_len][header bytes][u32 payload_len][payload bytes]
        let header_len = header_bytes.len() as u32;
        let payload_len = payload.len() as u32;

        let mut frame =
            Vec::with_capacity(8 + header_bytes.len() + payload.len());
        frame.extend_from_slice(&header_len.to_be_bytes());
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&payload_len.to_be_bytes());
        frame.extend_from_slice(payload);

        self.stream.write_all(&frame).map_err(|e| {
            if e.kind() == std::io::ErrorKind::BrokenPipe
                || e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::UnexpectedEof
            {
                TransportError::Disconnected
            } else {
                TransportError::Io(e)
            }
        })
    }

    /// Receive one complete packet from the TCP stream.
    ///
    /// Blocks until a full header+payload pair is available.
    ///
    /// # Errors
    ///
    /// - [`TransportError::Disconnected`] if the remote closed cleanly (EOF).
    /// - [`TransportError::Io`] on I/O errors.
    /// - [`TransportError::HeaderTooLarge`] if the announced header size
    ///   exceeds [`MAX_HEADER_BYTES`].
    /// - [`TransportError::PayloadTooLarge`] if the announced payload size
    ///   exceeds [`MAX_PAYLOAD_BYTES`].
    /// - [`TransportError::DeserialiseError`] if the header bytes are invalid.
    fn recv(&mut self) -> Result<(PacketHeader, Vec<u8>), TransportError> {
        // --- Step 1: Read header length (4 bytes) ---
        let header_len = read_u32(&mut self.stream)?;
        if header_len > MAX_HEADER_BYTES {
            return Err(TransportError::HeaderTooLarge(header_len, MAX_HEADER_BYTES));
        }

        // --- Step 2: Read header bytes ---
        let mut header_buf = vec![0u8; header_len];
        read_exact(&mut self.stream, &mut header_buf)?;

        // --- Step 3: Deserialise header ---
        let header = decode_header(&header_buf)?;

        // --- Step 4: Read payload length (4 bytes) ---
        let payload_len = read_u32(&mut self.stream)?;
        if payload_len > MAX_PAYLOAD_BYTES {
            return Err(TransportError::PayloadTooLarge(payload_len, MAX_PAYLOAD_BYTES));
        }

        // --- Step 5: Read payload bytes ---
        let mut payload = vec![0u8; payload_len];
        read_exact(&mut self.stream, &mut payload)?;

        Ok((header, payload))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Read exactly 4 bytes from `stream` and interpret them as a big-endian `u32`.
///
/// Returns [`TransportError::Disconnected`] on clean EOF (zero bytes read),
/// or [`TransportError::Io`] on other errors.
fn read_u32(stream: &mut TcpStream) -> Result<usize, TransportError> {
    let mut buf = [0u8; 4];
    read_exact(stream, &mut buf)?;
    Ok(u32::from_be_bytes(buf) as usize)
}

/// Read exactly `buf.len()` bytes from `stream`.
///
/// Unlike `stream.read()`, this function loops until the buffer is full or
/// an error occurs. This is essential for TCP, which may deliver data in
/// smaller chunks than requested.
///
/// Returns [`TransportError::Disconnected`] on clean EOF,
/// [`TransportError::Io`] on I/O errors.
fn read_exact(stream: &mut TcpStream, buf: &mut [u8]) -> Result<(), TransportError> {
    stream.read_exact(buf).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof
            || e.kind() == std::io::ErrorKind::ConnectionReset
        {
            TransportError::Disconnected
        } else {
            TransportError::Io(e)
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PacketType;
    use std::net::TcpListener;
    use std::thread;

    /// Test that a packet sent through a real TcpStream arrives intact.
    ///
    /// This test spins up a local listener on an ephemeral port, sends one
    /// packet from one thread, and verifies the other receives it correctly.
    #[test]
    fn roundtrip_over_real_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed");
        let addr = listener.local_addr().expect("local_addr failed");

        let header_sent = PacketHeader {
            dst_path: "/agents/test/shell".into(),
            src_path: "/operator/sess1".into(),
            packet_type: PacketType::Request,
        };
        let payload_sent = b"hello world".to_vec();

        let header_clone = header_sent.clone();
        let payload_clone = payload_sent.clone();

        // Sender thread
        let sender = thread::spawn(move || {
            let stream = TcpStream::connect(addr).expect("connect failed");
            let mut transport = TcpTransport::from_stream(stream);
            transport
                .send(&header_clone, &payload_clone)
                .expect("send failed");
        });

        // Receiver (main thread)
        let (stream, _) = listener.accept().expect("accept failed");
        let mut transport = TcpTransport::from_stream(stream);
        let (header_recv, payload_recv) = transport.recv().expect("recv failed");

        sender.join().expect("sender thread panicked");

        assert_eq!(header_recv.dst_path, header_sent.dst_path);
        assert_eq!(header_recv.src_path, header_sent.src_path);
        assert_eq!(header_recv.packet_type, header_sent.packet_type);
        assert_eq!(payload_recv, payload_sent);
    }

    /// Test that an empty payload round-trips correctly.
    #[test]
    fn roundtrip_empty_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed");
        let addr = listener.local_addr().expect("local_addr failed");

        let header = PacketHeader {
            dst_path: "/router/ping".into(),
            src_path: "/operator/sess1".into(),
            packet_type: PacketType::Request,
        };

        let header_clone = header.clone();
        let sender = thread::spawn(move || {
            let stream = TcpStream::connect(addr).expect("connect failed");
            let mut t = TcpTransport::from_stream(stream);
            t.send(&header_clone, &[]).expect("send failed");
        });

        let (stream, _) = listener.accept().expect("accept failed");
        let mut t = TcpTransport::from_stream(stream);
        let (recv_header, recv_payload) = t.recv().expect("recv failed");

        sender.join().expect("sender thread panicked");

        assert_eq!(recv_header.dst_path, "/router/ping");
        assert!(recv_payload.is_empty());
    }

    /// Test that a large payload (1 MB) survives the TCP framing.
    #[test]
    fn roundtrip_large_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed");
        let addr = listener.local_addr().expect("local_addr failed");

        let payload: Vec<u8> = (0..1_000_000u32).map(|i| (i % 256) as u8).collect();
        let payload_clone = payload.clone();

        let header = PacketHeader {
            dst_path: "/agents/x/files/read".into(),
            src_path: "/operator/sess1".into(),
            packet_type: PacketType::Response,
        };
        let header_clone = header.clone();

        let sender = thread::spawn(move || {
            let stream = TcpStream::connect(addr).expect("connect failed");
            let mut t = TcpTransport::from_stream(stream);
            t.send(&header_clone, &payload_clone).expect("send failed");
        });

        let (stream, _) = listener.accept().expect("accept failed");
        let mut t = TcpTransport::from_stream(stream);
        let (_, recv_payload) = t.recv().expect("recv failed");

        sender.join().expect("sender thread panicked");

        assert_eq!(recv_payload, payload);
    }

    /// Test that a frame whose announced header size exceeds the limit is rejected
    /// without allocating the full buffer.
    #[test]
    fn rejects_oversized_header() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed");
        let addr = listener.local_addr().expect("local_addr failed");

        let sender = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect failed");
            // Write an enormous header length
            let huge_len = (MAX_HEADER_BYTES + 1) as u32;
            stream
                .write_all(&huge_len.to_be_bytes())
                .expect("write failed");
        });

        let (stream, _) = listener.accept().expect("accept failed");
        let mut t = TcpTransport::from_stream(stream);
        let result = t.recv();

        sender.join().expect("sender panicked");

        assert!(
            matches!(result, Err(TransportError::HeaderTooLarge(_, _))),
            "expected HeaderTooLarge, got: {result:?}"
        );
    }
}
