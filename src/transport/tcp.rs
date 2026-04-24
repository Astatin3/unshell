//! TCP framed transport.

use alloc::vec::Vec;
use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
};

use crate::{
    protocol::FrameBytes,
    transport::{MAX_HEADER_BYTES, MAX_PAYLOAD_BYTES, Transport, TransportError},
};

/// Framed TCP transport.
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Connects to a remote address.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the TCP connection cannot be established.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, TransportError> {
        Ok(Self {
            stream: TcpStream::connect(addr)?,
        })
    }

    /// Wraps an existing TCP stream.
    pub fn from_stream(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl Transport for TcpTransport {
    fn send_frame(&mut self, frame: FrameBytes) -> Result<(), TransportError> {
        self.stream.write_all(&frame).map_err(map_io_error)
    }

    fn recv_frame(&mut self) -> Result<FrameBytes, TransportError> {
        let header_len = read_u32(&mut self.stream)?;
        if header_len > MAX_HEADER_BYTES {
            return Err(TransportError::HeaderTooLarge(header_len, MAX_HEADER_BYTES));
        }

        let mut header = vec![0u8; header_len];
        read_exact(&mut self.stream, &mut header)?;

        let payload_len = read_u32(&mut self.stream)?;
        if payload_len > MAX_PAYLOAD_BYTES {
            return Err(TransportError::PayloadTooLarge(
                payload_len,
                MAX_PAYLOAD_BYTES,
            ));
        }

        let mut payload = vec![0u8; payload_len];
        read_exact(&mut self.stream, &mut payload)?;

        let mut frame = Vec::with_capacity(8 + header_len + payload_len);
        frame.extend_from_slice(&(header_len as u32).to_be_bytes());
        frame.extend_from_slice(&header);
        frame.extend_from_slice(&(payload_len as u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame.into_boxed_slice())
    }
}

fn read_u32(stream: &mut TcpStream) -> Result<usize, TransportError> {
    let mut bytes = [0u8; 4];
    read_exact(stream, &mut bytes)?;
    Ok(u32::from_be_bytes(bytes) as usize)
}

fn read_exact(stream: &mut TcpStream, buffer: &mut [u8]) -> Result<(), TransportError> {
    stream.read_exact(buffer).map_err(map_io_error)
}

fn map_io_error(error: std::io::Error) -> TransportError {
    match error.kind() {
        ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::ConnectionReset => {
            TransportError::Disconnected
        }
        _ => TransportError::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DataMessage, PacketHeader, PacketType, decode_frame, encode_packet};
    use alloc::{string::String, vec};
    use std::{net::TcpListener, thread};

    #[test]
    fn tcp_roundtrip_preserves_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("local address should exist");

        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: vec![String::from("a")],
            dst_path: vec![String::from("b")],
            dst_leaf: None,
            hook_id: Some(9),
        };
        let payload = DataMessage {
            procedure_id: String::from("org.product.v1.echo.roundtrip"),
            data: b"payload".to_vec(),
            end_hook: true,
        };
        let frame = encode_packet(&header, &payload).expect("frame should encode");

        let sender = thread::spawn(move || {
            let mut transport = TcpTransport::connect(addr).expect("connect should succeed");
            transport.send_frame(frame).expect("send should succeed");
        });

        let (stream, _) = listener.accept().expect("accept should succeed");
        let mut transport = TcpTransport::from_stream(stream);
        let received = transport.recv_frame().expect("recv should succeed");
        let parsed = decode_frame(&received).expect("frame should decode");

        sender.join().expect("sender should not panic");
        assert_eq!(
            parsed.deserialize_data().expect("data should decode"),
            payload
        );
    }
}
