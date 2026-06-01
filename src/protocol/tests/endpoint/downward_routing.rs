use alloc::vec;

use crate::protocol::{Endpoint, EndpointError, RouteDirection};

use super::super::support::{
    assertions::{assert_hook_present, assert_hook_removed},
    endpoints::{ENDPOINT_A, ENDPOINT_B, ENDPOINT_C, endpoint_at, single_outbound_packet},
    packets::{echo_packet, echo_packet_with_end},
};

#[test]
fn inbound_downward_packet_routes_to_immediate_child() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.add_connection(ENDPOINT_A, true);
    endpoint.add_connection(ENDPOINT_C, false);

    endpoint
        .add_inbound_from(
            ENDPOINT_A,
            echo_packet(vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C], hook_id),
        )
        .unwrap();

    let packet = single_outbound_packet(&endpoint, ENDPOINT_C);
    assert!(!packet.end_hook);
    assert_eq!(packet.path, vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C]);
    assert_hook_present(&endpoint, hook_id);
    assert_eq!(endpoint.hook_peer(hook_id), Some(ENDPOINT_C));
    assert!(!Endpoint::route_contains(ENDPOINT_A, &endpoint.outbound));
}

#[test]
fn outbound_downward_packet_routes_to_immediate_child() {
    let mut endpoint = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_B);
    endpoint.add_connection(ENDPOINT_B, false);

    endpoint
        .add_outbound(echo_packet_with_end(
            vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C],
            hook_id,
            true,
        ))
        .unwrap();

    let packet = single_outbound_packet(&endpoint, ENDPOINT_B);
    assert!(packet.end_hook);
    assert_eq!(packet.path, vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C]);
    assert_hook_removed(&endpoint, hook_id);
    assert!(!Endpoint::route_contains(ENDPOINT_C, &endpoint.outbound));
}

#[test]
fn downward_outbound_without_hook_is_allowed() {
    let mut endpoint = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    endpoint.add_connection(ENDPOINT_B, false);

    let new_hook = endpoint.get_hook_id();

    endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A, ENDPOINT_B], new_hook))
        .unwrap();

    assert_eq!(
        Endpoint::route_get(ENDPOINT_B, &endpoint.outbound)
            .unwrap()
            .len(),
        1
    );
    assert_hook_present(&endpoint, new_hook);
    assert_eq!(endpoint.hook_peer(new_hook), Some(ENDPOINT_B));
}

#[test]
fn downward_route_without_connection_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    let hook_id = endpoint.get_hook_id();

    let error = endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id))
        .unwrap_err();

    assert!(matches!(
        error,
        EndpointError::MissingConnection {
            next_hop: ENDPOINT_B,
            direction: RouteDirection::Downward,
        }
    ));
    assert_hook_removed(&endpoint, hook_id);
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}
