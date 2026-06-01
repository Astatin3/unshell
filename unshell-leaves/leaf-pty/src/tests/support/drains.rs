use alloc::vec::Vec;

use unshell::protocol::{Endpoint, Packet};

use crate::PROC_PTY;

use super::ENDPOINT_A;

/// Drains packets for `procedure_id` delivered to endpoint A.
pub(crate) fn drain_parent_packets(endpoint: &mut Endpoint, procedure_id: u32) -> Vec<Packet> {
    let mut packets = Vec::new();
    endpoint.take_inbound_matching(
        ENDPOINT_A,
        |packet| packet.procedure_id == procedure_id,
        |packet| packets.push(packet),
    );
    packets
}

/// Drains PTY packets delivered to endpoint A.
pub(crate) fn drain_parent_pty_packets(endpoint: &mut Endpoint) -> Vec<Packet> {
    drain_parent_packets(endpoint, PROC_PTY)
}
