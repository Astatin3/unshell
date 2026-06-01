use alloc::vec;

use crate::protocol::{Endpoint, EndpointError};

use super::super::support::{
    assertions::{assert_hook_present, assert_hook_removed},
    endpoints::{ENDPOINT_A, ENDPOINT_B, endpoint_at, single_inbound_packet},
    packets::echo_packet,
};

#[test]
fn inbound_downward_packet_for_local_endpoint_opens_hook() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.add_connection(ENDPOINT_A, true);

    endpoint
        .add_inbound_from(
            ENDPOINT_A,
            echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id),
        )
        .unwrap();

    let packet = single_inbound_packet(&endpoint, ENDPOINT_B);
    assert!(!packet.end_hook);
    assert_eq!(packet.path, vec![ENDPOINT_A, ENDPOINT_B]);
    assert_hook_present(&endpoint, hook_id);
    assert_eq!(endpoint.hook_peer(hook_id), Some(ENDPOINT_A));
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn outbound_packet_for_local_endpoint_is_delivered_locally() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();

    endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id))
        .unwrap();

    let packet = single_inbound_packet(&endpoint, ENDPOINT_B);
    assert!(!packet.end_hook);
    assert_eq!(packet.data, "ABC123".as_bytes());
    assert_hook_removed(&endpoint, hook_id);
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn inbound_without_absolute_path_is_rejected() {
    let mut endpoint = Endpoint::new(ENDPOINT_A);

    let error = endpoint
        .add_inbound(echo_packet(vec![ENDPOINT_A], 1))
        .unwrap_err();

    assert!(matches!(error, EndpointError::EndpointPathUnset));
    assert!(Endpoint::routes_is_empty(&endpoint.inbound));
}

#[test]
fn outbound_without_absolute_path_is_rejected() {
    let mut endpoint = Endpoint::new(ENDPOINT_A);

    let error = endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A], 1))
        .unwrap_err();

    assert!(matches!(error, EndpointError::EndpointPathUnset));
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}
