use unshell::protocol::Leaf;

use crate::{FakePtyLeaf, FakePtyState, OP_INPUT, OP_OUTPUT};

use super::super::support::{
    ENDPOINT_A, ENDPOINT_B, assert_hook_present, drain_parent_pty_packets, has_frame,
    open_pty_session, pty_endpoints, send_downward_frame, transfer_packets,
};

#[test]
fn two_pty_sessions_interleave_without_crossing_hooks() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());

    let first_hook = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    let second_hook = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        second_hook,
        OP_INPUT,
        b"second",
        false,
    );
    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        first_hook,
        OP_INPUT,
        b"first",
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(leaf.active_session_count(), 2);
    assert_eq!(packets.len(), 2);
    assert!(has_frame(&packets, first_hook, OP_OUTPUT, b"first"));
    assert!(has_frame(&packets, second_hook, OP_OUTPUT, b"second"));
    assert_hook_present(&endpoint_a, first_hook);
    assert_hook_present(&endpoint_a, second_hook);
    assert_hook_present(&endpoint_b, first_hook);
    assert_hook_present(&endpoint_b, second_hook);
}
