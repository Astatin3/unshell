//! Framed transport implementations.
//!
//! Transports move complete framed packets represented by [`crate::protocol::FrameBytes`].
//! Packet parsing and validation live above this layer.

use crate::protocol::FrameBytes;

#[cfg(feature = "sim")]
pub mod channel;
#[cfg(feature = "tcp")]
pub mod tcp;

/// Maximum allowed size for a serialized header section.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Maximum allowed size for a serialized payload section.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

/// Transport-layer failure.
#[derive(Debug)]
pub enum TransportError {
    /// The peer disconnected cleanly.
    Disconnected,
    /// The announced header length exceeded the limit.
    HeaderTooLarge(usize, usize),
    /// The announced payload length exceeded the limit.
    PayloadTooLarge(usize, usize),
    /// Underlying I/O failure.
    #[cfg(feature = "tcp")]
    Io(std::io::Error),
    /// Channel send or receive failure.
    #[cfg(feature = "sim")]
    ChannelClosed,
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Disconnected => f.write_str("transport disconnected"),
            Self::HeaderTooLarge(got, max) => {
                write!(f, "header too large: {got} bytes (limit {max})")
            }
            Self::PayloadTooLarge(got, max) => {
                write!(f, "payload too large: {got} bytes (limit {max})")
            }
            #[cfg(feature = "tcp")]
            Self::Io(error) => write!(f, "transport I/O error: {error}"),
            #[cfg(feature = "sim")]
            Self::ChannelClosed => f.write_str("channel transport closed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportError {}

#[cfg(feature = "tcp")]
impl From<std::io::Error> for TransportError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Duplex framed transport.
pub trait Transport: Send {
    /// Sends one complete framed packet.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the underlying transport cannot deliver the frame.
    fn send_frame(&mut self, frame: FrameBytes) -> Result<(), TransportError>;

    /// Receives one complete framed packet.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the transport disconnects or a frame cannot be read.
    fn recv_frame(&mut self) -> Result<FrameBytes, TransportError>;
}
