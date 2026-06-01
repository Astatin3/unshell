use alloc::vec;

use unshell::protocol::{Endpoint, Leaf};

use crate::{FakePtyLeaf, pty_open_packet, pty_packet};

use super::{ENDPOINT_A, ENDPOINT_B, transfer_packets};

/// Sends one downward PTY frame from endpoint A to endpoint B.
pub(crate) fn send_downward_frame(
    endpoint_a: &mut Endpoint,
    endpoint_b: &mut Endpoint,
    hook_id: u16,
    opcode: u8,
    payload: &[u8],
    end_hook: bool,
) {
    endpoint_a
        .add_outbound(pty_packet(
            vec![ENDPOINT_A, ENDPOINT_B],
            hook_id,
            end_hook,
            opcode,
            payload,
        ))
        .unwrap();
    transfer_packets(endpoint_a, endpoint_b, ENDPOINT_B, ENDPOINT_A);
}

/// Opens a fake PTY session and delivers the `Opened` response to endpoint A.
pub(crate) fn open_pty_session(
    endpoint_a: &mut Endpoint,
    endpoint_b: &mut Endpoint,
    leaf: &mut FakePtyLeaf,
) -> u16 {
    let hook_id = endpoint_a.get_hook_id();
    endpoint_a
        .add_outbound(pty_open_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id))
        .unwrap();

    transfer_packets(endpoint_a, endpoint_b, ENDPOINT_B, ENDPOINT_A);
    leaf.update(endpoint_b);
    transfer_packets(endpoint_b, endpoint_a, ENDPOINT_A, ENDPOINT_B);

    hook_id
}
