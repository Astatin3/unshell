use unshell::protocol::Leaf;

use crate::{
    FakePtyLeaf, FakePtyState, OP_ABORT, OP_ERROR, OP_EXIT, OP_INPUT, OP_STDIN_EOF, OP_TERMINATE,
};

use super::super::support::{
    ENDPOINT_A, ENDPOINT_B, assert_frame, assert_hook_present, assert_hook_removed, assert_opened,
    drain_parent_pty_packets, open_pty_session, pty_endpoints, send_downward_frame,
    transfer_packets,
};

#[test]
fn open_pty_paves_hook_and_creates_session() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());

    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(leaf.active_session_count(), 1);
    assert_eq!(leaf.state().active_count, 1);
    assert_eq!(leaf.state().total_opened, 1);
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_present(&endpoint_b, hook_id);
    assert_eq!(packets.len(), 1);
    assert_opened(&packets[0], hook_id);
}

#[test]
fn stdin_eof_keeps_hook_until_exit() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_STDIN_EOF,
        &[],
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);

    assert_eq!(leaf.state().last_stdin_eof_hook, Some(hook_id));
    assert!(drain_parent_pty_packets(&mut endpoint_a).is_empty());
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_present(&endpoint_b, hook_id);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_TERMINATE,
        &[],
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_EXIT, true, &[0]);
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn exit_end_hook_cleans_route_and_session() {
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
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_EXIT, true, &[0]);
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn abort_downward_end_hook_closes_without_ack() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_ABORT,
        &[],
        true,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);

    assert_eq!(leaf.active_session_count(), 0);
    assert!(drain_parent_pty_packets(&mut endpoint_a).is_empty());
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn unknown_session_input_returns_error_end_hook() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = endpoint_a.get_hook_id();

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_INPUT,
        b"orphan",
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_ERROR, true, b"unknown-session");
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}
