//! Simulated transport built on `crossbeam-channel`.

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::{
    protocol::FrameBytes,
    transport::{Transport, TransportError},
};

/// One endpoint of a simulated duplex transport.
#[derive(Debug, Clone)]
pub struct ChannelTransport {
    sender: Sender<FrameBytes>,
    receiver: Receiver<FrameBytes>,
}

impl ChannelTransport {
    /// Builds a connected pair of transports.
    pub fn pair() -> (Self, Self) {
        let (ab_tx, ab_rx) = unbounded();
        let (ba_tx, ba_rx) = unbounded();
        (
            Self {
                sender: ab_tx,
                receiver: ba_rx,
            },
            Self {
                sender: ba_tx,
                receiver: ab_rx,
            },
        )
    }
}

impl Transport for ChannelTransport {
    fn send_frame(&mut self, frame: FrameBytes) -> Result<(), TransportError> {
        self.sender
            .send(frame)
            .map_err(|_| TransportError::ChannelClosed)
    }

    fn recv_frame(&mut self) -> Result<FrameBytes, TransportError> {
        self.receiver
            .recv()
            .map_err(|_| TransportError::ChannelClosed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DataMessage, PacketHeader, PacketType, decode_frame, encode_packet};
    use alloc::{string::String, vec};

    #[test]
    fn channel_roundtrip_moves_framed_bytes() {
        let (mut left, mut right) = ChannelTransport::pair();
        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: vec![String::from("a")],
            dst_path: vec![String::from("b")],
            dst_leaf: None,
            hook_id: Some(7),
        };
        let data = DataMessage {
            procedure_id: String::from("org.product.v1.echo.roundtrip"),
            data: b"payload".to_vec(),
            end_hook: true,
        };
        let frame = encode_packet(&header, &data).expect("frame should encode");

        left.send_frame(frame).expect("send should succeed");
        let received = right.recv_frame().expect("recv should succeed");
        let parsed = decode_frame(&received).expect("received frame should decode");
        assert_eq!(parsed.deserialize_data().expect("data should decode"), data);
    }
}
