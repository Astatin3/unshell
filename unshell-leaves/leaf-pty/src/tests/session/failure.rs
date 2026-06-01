use unshell::protocol::Leaf;

use crate::{FakePtyLeaf, FakePtyState, OP_TERMINATE};

use super::super::support::{
    ENDPOINT_A, ENDPOINT_B, assert_hook_present, assert_hook_removed, drain_parent_pty_packets,
    open_pty_session, pty_endpoints, send_downward_frame, transfer_packets,
};

#[test]
fn failed_final_exit_route_closes_session_without_retry() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_TERMINATE,
        &[],
        false,
    );
    endpoint_b.remove_connection(ENDPOINT_A, true);
    leaf.update(&mut endpoint_b);

    assert_eq!(leaf.active_session_count(), 0);
    assert_eq!(leaf.pending_packet_count(), 0);
    assert_hook_removed(&endpoint_b, hook_id);

    endpoint_b.add_connection(ENDPOINT_A, true);
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert!(packets.is_empty());
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}
