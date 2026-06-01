use unshell::protocol::Leaf;

use crate::{FakePtyLeaf, FakePtyState, OP_INPUT, OP_OUTPUT};

use super::super::support::{
    ENDPOINT_A, ENDPOINT_B, assert_frame, assert_hook_present, drain_parent_pty_packets,
    open_pty_session, pty_endpoints, send_downward_frame, transfer_packets,
};

#[test]
fn input_and_output_share_one_hook() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_INPUT,
        b"hello",
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_OUTPUT, false, b"hello");
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_present(&endpoint_b, hook_id);
}
