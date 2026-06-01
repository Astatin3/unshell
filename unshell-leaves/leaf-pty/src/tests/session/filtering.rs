use alloc::{vec, vec::Vec};

use unshell::protocol::{Leaf, Packet};

use crate::{FakePtyLeaf, FakePtyState, pty_open_packet};

use super::super::support::{ENDPOINT_A, ENDPOINT_B, PROC_OTHER, endpoint_at};

#[test]
fn pty_leaf_does_not_consume_other_leaf_packets() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    endpoint.add_connection(ENDPOINT_A, true);

    endpoint
        .add_inbound_from(ENDPOINT_A, pty_open_packet(vec![ENDPOINT_A, ENDPOINT_B], 7))
        .unwrap();
    endpoint
        .add_inbound_from(
            ENDPOINT_A,
            Packet {
                hook_id: 8,
                end_hook: false,
                path: vec![ENDPOINT_A, ENDPOINT_B],
                procedure_id: PROC_OTHER,
                data: b"leave-me".to_vec(),
            },
        )
        .unwrap();

    leaf.update(&mut endpoint);

    let mut other_packets = Vec::new();
    endpoint.take_inbound_matching(
        ENDPOINT_B,
        |packet| packet.procedure_id == PROC_OTHER,
        |packet| other_packets.push(packet),
    );

    assert_eq!(leaf.active_session_count(), 1);
    assert_eq!(other_packets.len(), 1);
    assert_eq!(other_packets[0].procedure_id, PROC_OTHER);
    assert_eq!(other_packets[0].data, b"leave-me".to_vec());
}
