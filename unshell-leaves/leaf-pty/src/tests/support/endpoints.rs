use alloc::{vec, vec::Vec};

use unshell::protocol::{Endpoint, Packet};

pub(crate) const ENDPOINT_A: u32 = 0;
pub(crate) const ENDPOINT_B: u32 = 1;
pub(crate) const PROC_OTHER: u32 = 31;

/// Creates a bare endpoint at a known absolute path.
pub(crate) fn endpoint_at(id: u32, path: Vec<u32>) -> Endpoint {
    let mut endpoint = Endpoint::new(id);
    endpoint.path = path;
    endpoint
}

/// Creates the parent/child endpoint pair used by PTY session tests.
pub(crate) fn pty_endpoints() -> (Endpoint, Endpoint) {
    let mut endpoint_a = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    let mut endpoint_b = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);

    endpoint_a.add_connection(ENDPOINT_B, false);
    endpoint_b.add_connection(ENDPOINT_A, true);

    (endpoint_a, endpoint_b)
}

/// Transfers every queued packet for `next_hop` into `receiver` as `remote_id` traffic.
pub(crate) fn transfer_packets(
    sender: &mut Endpoint,
    receiver: &mut Endpoint,
    next_hop: u32,
    remote_id: u32,
) {
    let mut packets = Vec::<Packet>::new();
    sender.take_outbound_clear(next_hop, |packet| packets.push(packet.clone()));

    for packet in packets {
        receiver.add_inbound_from(remote_id, packet).unwrap();
    }
}
