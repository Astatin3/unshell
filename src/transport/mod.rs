//! # Transport Module
//!
//! The transport layer abstracts the network connection used to carry protocol packets.
//!
//! ## Module layout
//!
//! ```text
//! transport/
//!   mod.rs   ← you are here; Transport trait, TransportError, frame encoding
//!   tcp.rs   ← TcpTransport: Transport implemented for std::net::TcpStream
//! ```
//!
//! ## Design
//!
//! A `Transport` sends and receives complete logical packets. Each packet is
//! one `PacketHeader` + one opaque payload byte slice.
//!
//! Internally, implementations must use the two-part framing format:
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────┐
//! │  [u32 big-endian header_len][header bytes][u32 big-endian pay_len]  │
//! │  [payload bytes]                                                     │
//! └──────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! **IMPORTANT:** TCP is a stream protocol. A single `read()` call may return
//! fewer bytes than requested. All receive operations MUST loop until the
//! exact number of bytes has been read. The standard pattern is `read_exact()`.
//!
//! ## Size limits
//!
//! | Limit | Value | Reason |
//! |---|---|---|
//! | Max header bytes | 64 KB | Headers are always small; larger = bug or attack |
//! | Max payload bytes | 64 MB | Sufficient for most file transfers |
//!
//! ## Transport implementations
//!
//! | Type | Where | Description |
//! |---|---|---|
//! | [`tcp::TcpTransport`] | `transport/tcp.rs` | Standard TCP socket |
//!
//! Future additions: `HttpsTransport`, `IcmpTransport`, `OpenVpnTransport`.

extern crate alloc;
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;

use crate::protocol::PacketHeader;

/// TCP transport implementation.
///
/// Only available when the `tcp` feature is enabled (requires `std`).
/// Enable with `unshell = { features = ["tcp"] }` in your `Cargo.toml`.
#[cfg(feature = "tcp")]
pub mod tcp;

// ---------------------------------------------------------------------------
// Frame size limits
// ---------------------------------------------------------------------------

/// Maximum allowed size for a serialised `PacketHeader` (64 KB).
///
/// Headers should be tiny (< 200 bytes in practice). Anything larger suggests
/// either a bug in the sender or a malformed/malicious frame.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Maximum allowed size for a packet payload (64 MB).
///
/// Sufficient for most file transfers without chunking.
/// Larger transfers will require the (not-yet-implemented) streaming extension.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// TransportError
// ---------------------------------------------------------------------------

/// Errors that can occur during [`Transport`] operations.
///
/// # Reconnect policy
///
/// When a payload receives [`TransportError::Disconnected`] or
/// [`TransportError::Io`], it should:
/// 1. Close the current transport.
/// 2. Wait 5 seconds.
/// 3. Attempt to create a new transport connection.
/// 4. Repeat indefinitely on failure.
///
/// The operator CLI exits on disconnect (the user restarts it manually).
#[derive(Debug)]
pub enum TransportError {
    /// An I/O error from the underlying stream.
    ///
    /// This includes partial writes, socket errors, and OS-level failures.
    /// Only available when the `tcp` feature is enabled (requires std).
    #[cfg(feature = "tcp")]
    Io(std::io::Error),

    /// The announced frame header length exceeds [`MAX_HEADER_BYTES`].
    ///
    /// The connection should be closed immediately — the remote end is either
    /// buggy or malicious. Do not allocate a buffer of the announced size.
    ///
    /// Fields: `(announced_size, limit)`.
    HeaderTooLarge(usize, usize),

    /// The announced frame payload length exceeds [`MAX_PAYLOAD_BYTES`].
    ///
    /// Fields: `(announced_size, limit)`.
    PayloadTooLarge(usize, usize),

    /// The remote end closed the connection cleanly (EOF).
    ///
    /// This is not an error in the traditional sense. It means the other side
    /// disconnected intentionally (e.g., payload restarted, operator exited).
    Disconnected,

    /// The received bytes could not be deserialised as a `PacketHeader`.
    ///
    /// This indicates a protocol version mismatch or data corruption.
    DeserialiseError,
}

#[cfg(feature = "tcp")]
impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "transport I/O error: {e}"),
            Self::HeaderTooLarge(got, max) => {
                write!(f, "frame header too large: {got} bytes (limit: {max})")
            }
            Self::PayloadTooLarge(got, max) => {
                write!(f, "frame payload too large: {got} bytes (limit: {max})")
            }
            Self::Disconnected => write!(f, "connection closed by remote"),
            Self::DeserialiseError => write!(f, "failed to deserialise packet header"),
        }
    }
}

#[cfg(not(feature = "tcp"))]
impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HeaderTooLarge(got, max) => {
                write!(f, "frame header too large: {got} bytes (limit: {max})")
            }
            Self::PayloadTooLarge(got, max) => {
                write!(f, "frame payload too large: {got} bytes (limit: {max})")
            }
            Self::Disconnected => write!(f, "connection closed by remote"),
            Self::DeserialiseError => write!(f, "failed to deserialise packet header"),
        }
    }
}

#[cfg(feature = "tcp")]
impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// Implement std::error::Error so TransportError works with `?` in Box<dyn Error> contexts.
#[cfg(feature = "tcp")]
impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Transport trait
// ---------------------------------------------------------------------------

/// A bidirectional framed transport.
///
/// Implementors handle the low-level byte transfer, including framing,
/// length prefixes, and the `read_exact` loop. The protocol layer above
/// sees complete logical packets (header + payload pairs).
///
/// # Contract
///
/// - `send` must write all bytes before returning `Ok(())`.
/// - `recv` must block until a complete header+payload pair is available.
/// - Both methods must use `read_exact`-style loops (never a single `read`).
/// - Frame size checks must be performed before any allocation.
///
/// # Example: implementing a custom transport
///
/// ```rust,no_run
/// use unshell::transport::{Transport, TransportError};
/// use unshell::protocol::PacketHeader;
///
/// struct MyTransport { /* ... */ }
///
/// impl Transport for MyTransport {
///     fn send(&mut self, header: &PacketHeader, payload: &[u8])
///         -> Result<(), TransportError>
///     {
///         // 1. Serialise header with rkyv
///         // 2. Write [u32 header_len][header bytes][u32 payload_len][payload bytes]
///         // 3. Use write_all() — never plain write()
///         todo!()
///     }
///
///     fn recv(&mut self) -> Result<(PacketHeader, Vec<u8>), TransportError> {
///         // 1. read_exact 4 bytes → header_len
///         // 2. Check header_len <= MAX_HEADER_BYTES before allocating
///         // 3. read_exact header_len bytes
///         // 4. Deserialise header
///         // 5. read_exact 4 bytes → payload_len
///         // 6. Check payload_len <= MAX_PAYLOAD_BYTES before allocating
///         // 7. read_exact payload_len bytes
///         // 8. Return (header, payload)
///         todo!()
///     }
/// }
///
/// // SAFETY: MyTransport owns its stream exclusively and does not share it.
/// unsafe impl Send for MyTransport {}
/// ```
pub trait Transport: Send {
    /// Send one complete packet over this transport.
    ///
    /// Blocks until all bytes have been written.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the write fails partway through,
    /// or [`TransportError::Disconnected`] if the remote end is closed.
    fn send(&mut self, header: &PacketHeader, payload: &[u8]) -> Result<(), TransportError>;

    /// Receive one complete packet from this transport.
    ///
    /// Blocks until a full header+payload pair is available.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Disconnected`] if the remote closes cleanly,
    /// [`TransportError::Io`] on I/O errors, [`TransportError::HeaderTooLarge`]
    /// or [`TransportError::PayloadTooLarge`] if a size limit is exceeded,
    /// and [`TransportError::DeserialiseError`] if the header cannot be decoded.
    fn recv(&mut self) -> Result<(PacketHeader, Vec<u8>), TransportError>;
}

// ---------------------------------------------------------------------------
// Frame encoding helpers (shared by all transport implementations)
// ---------------------------------------------------------------------------

/// Encode a `PacketHeader` to bytes using rkyv.
///
/// Returns the serialised byte vector, or `None` if serialisation fails.
///
/// This is a low-level helper; transport implementations call it in `send()`.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{PacketHeader, PacketType};
/// use unshell::transport::encode_header;
///
/// let header = PacketHeader {
///     dst_path: "/router".into(),
///     src_path: "/agents/abc123".into(),
///     packet_type: PacketType::Handshake,
/// };
/// let bytes = encode_header(&header).expect("serialisation should not fail");
/// assert!(!bytes.is_empty());
/// ```
pub fn encode_header(header: &PacketHeader) -> Option<Vec<u8>> {
    rkyv::to_bytes::<rkyv::rancor::Error>(header).ok().map(|b| b.to_vec())
}

/// Decode a `PacketHeader` from rkyv bytes.
///
/// Returns `Err(TransportError::DeserialiseError)` if the bytes are invalid.
///
/// This is a low-level helper; transport implementations call it in `recv()`.
///
/// # Example
///
/// ```rust
/// use unshell::protocol::{PacketHeader, PacketType};
/// use unshell::transport::{encode_header, decode_header};
///
/// let header = PacketHeader {
///     dst_path: "/router".into(),
///     src_path: "/agents/abc123".into(),
///     packet_type: PacketType::Handshake,
/// };
/// let bytes = encode_header(&header).unwrap();
/// let decoded = decode_header(&bytes).unwrap();
/// assert_eq!(decoded.dst_path, "/router");
/// ```
pub fn decode_header(bytes: &[u8]) -> Result<PacketHeader, TransportError> {
    rkyv::from_bytes::<PacketHeader, rkyv::rancor::Error>(bytes)
        .map_err(|_| TransportError::DeserialiseError)
}
