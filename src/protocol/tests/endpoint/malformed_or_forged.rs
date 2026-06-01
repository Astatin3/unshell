use alloc::vec;

use crate::protocol::{Endpoint, EndpointError, Leaf};

use super::super::support::{
    assertions::assert_hook_present,
    endpoints::{ENDPOINT_A, ENDPOINT_B, ENDPOINT_C, endpoint_at, single_inbound_packet},
    packets::{echo_packet, echo_packet_with_end},
    transport::CommsLeaf,
};

#[test]
fn forged_sideways_packet_is_rejected_as_incorrect_path() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_A);
    endpoint.add_connection(ENDPOINT_A, true);

    let error = endpoint
        .add_inbound_from(
            ENDPOINT_A,
            echo_packet(vec![ENDPOINT_A, ENDPOINT_C], hook_id),
        )
        .unwrap_err();

    assert!(matches!(error, EndpointError::DestinationOutsideLocalTree));
    assert_hook_present(&endpoint, hook_id);
    assert!(Endpoint::routes_is_empty(&endpoint.inbound));
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn malformed_frame_is_dropped_by_comms_leaf() {
    let (tx_to_endpoint, rx_for_endpoint) = crossbeam_channel::unbounded();
    let (tx_unused, _rx_unused) = crossbeam_channel::unbounded();
    let mut endpoint = Endpoint::new(ENDPOINT_B);
    let mut comms = CommsLeaf {
        tx: tx_unused,
        rx: rx_for_endpoint,
        remote_id: ENDPOINT_A,
        is_authority: true,
        started: false,
    };
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];

    tx_to_endpoint.send(vec![0, 1, 2, 3]).unwrap();
    comms.update(&mut endpoint);

    assert!(Endpoint::routes_is_empty(&endpoint.inbound));
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn malformed_frame_does_not_block_following_valid_packet() {
    let (tx_to_endpoint, rx_for_endpoint) = crossbeam_channel::unbounded();
    let (tx_unused, _rx_unused) = crossbeam_channel::unbounded();
    let hook_id = 42;
    let mut endpoint = Endpoint::new(ENDPOINT_B);
    let mut comms = CommsLeaf {
        tx: tx_unused,
        rx: rx_for_endpoint,
        remote_id: ENDPOINT_A,
        is_authority: true,
        started: false,
    };
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];

    tx_to_endpoint.send(vec![0, 1, 2, 3]).unwrap();
    tx_to_endpoint
        .send(
            echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id)
                .serialize()
                .unwrap(),
        )
        .unwrap();
    comms.update(&mut endpoint);

    let packet = single_inbound_packet(&endpoint, ENDPOINT_B);
    assert!(!packet.end_hook);
    assert_eq!(packet.hook_id, hook_id);
    assert_hook_present(&endpoint, hook_id);
}

#[test]
fn forged_frame_without_required_hook_is_dropped_by_comms_leaf() {
    let (tx_to_endpoint, rx_for_endpoint) = crossbeam_channel::unbounded();
    let (tx_unused, _rx_unused) = crossbeam_channel::unbounded();
    let mut endpoint = Endpoint::new(ENDPOINT_B);
    let mut comms = CommsLeaf {
        tx: tx_unused,
        rx: rx_for_endpoint,
        remote_id: ENDPOINT_C,
        is_authority: false,
        started: false,
    };
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];
    endpoint.accept_hook(7, ENDPOINT_C);
    endpoint.add_connection(ENDPOINT_A, true);

    tx_to_endpoint
        .send(
            echo_packet_with_end(vec![ENDPOINT_A], 12, true)
                .serialize()
                .unwrap(),
        )
        .unwrap();
    comms.update(&mut endpoint);

    assert_hook_present(&endpoint, 7);
    assert!(Endpoint::routes_is_empty(&endpoint.inbound));
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}
