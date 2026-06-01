use alloc::vec;

use crate::protocol::{Endpoint, EndpointError, RouteDirection};

use super::super::support::{
    assertions::{assert_hook_present, assert_hook_removed},
    endpoints::{ENDPOINT_A, ENDPOINT_B, ENDPOINT_C, endpoint_at, single_outbound_packet},
    packets::echo_packet_with_end,
};

#[test]
fn inbound_upward_packet_with_hook_routes_to_parent() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_C);
    endpoint.add_connection(ENDPOINT_A, true);
    endpoint.add_connection(ENDPOINT_C, false);

    endpoint
        .add_inbound_from(
            ENDPOINT_C,
            echo_packet_with_end(vec![ENDPOINT_A], hook_id, true),
        )
        .unwrap();

    let packet = single_outbound_packet(&endpoint, ENDPOINT_A);
    assert!(packet.end_hook);
    assert_eq!(packet.hook_id, hook_id);
    assert_hook_removed(&endpoint, hook_id);
    assert!(!Endpoint::route_contains(ENDPOINT_C, &endpoint.outbound));
}

#[test]
fn inbound_upward_packet_without_hook_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.add_connection(ENDPOINT_A, true);
    endpoint.add_connection(ENDPOINT_C, false);

    let error = endpoint
        .add_inbound_from(
            ENDPOINT_C,
            echo_packet_with_end(vec![ENDPOINT_A], hook_id, true),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        EndpointError::UnknownHook { hook_id: observed_hook_id } if observed_hook_id == hook_id
    ));
    assert!(Endpoint::routes_is_empty(&endpoint.inbound));
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn forged_upward_packet_with_unknown_hook_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    endpoint.accept_hook(7, ENDPOINT_C);
    endpoint.add_connection(ENDPOINT_A, true);
    endpoint.add_connection(ENDPOINT_C, false);

    let error = endpoint
        .add_inbound_from(ENDPOINT_C, echo_packet_with_end(vec![ENDPOINT_A], 99, true))
        .unwrap_err();

    assert!(matches!(error, EndpointError::UnknownHook { hook_id: 99 }));
    assert_hook_present(&endpoint, 7);
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn upward_outbound_without_hook_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    endpoint.accept_hook(7, ENDPOINT_A);
    endpoint.add_connection(ENDPOINT_A, true);

    let new_hook = endpoint.get_hook_id();

    let error = endpoint
        .add_outbound(echo_packet_with_end(vec![ENDPOINT_A], new_hook, true))
        .unwrap_err();

    assert!(matches!(
        error,
        EndpointError::UnknownHook { hook_id: observed_hook_id } if observed_hook_id == new_hook
    ));
    assert_hook_present(&endpoint, 7);
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn deeper_upward_route_uses_parent_as_next_hop() {
    let mut endpoint = endpoint_at(ENDPOINT_C, vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C]);
    let new_hook = endpoint.get_hook_id();

    endpoint.accept_hook(new_hook, ENDPOINT_B);
    endpoint.add_connection(ENDPOINT_B, true);

    endpoint
        .add_outbound(echo_packet_with_end(vec![ENDPOINT_A], new_hook, true))
        .unwrap();

    assert!(Endpoint::route_contains(ENDPOINT_B, &endpoint.outbound));
    assert!(!Endpoint::route_contains(ENDPOINT_A, &endpoint.outbound));
    assert_hook_removed(&endpoint, new_hook);
}

#[test]
fn upward_route_without_connection_is_rejected_even_with_hook() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_A);

    let error = endpoint
        .add_outbound(echo_packet_with_end(vec![ENDPOINT_A], hook_id, true))
        .unwrap_err();

    assert!(matches!(
        error,
        EndpointError::MissingConnection {
            next_hop: ENDPOINT_A,
            direction: RouteDirection::Upward,
        }
    ));
    assert_hook_present(&endpoint, hook_id);
    assert!(Endpoint::routes_is_empty(&endpoint.outbound));
}

#[test]
fn trusted_upward_packet_without_peer_metadata_checks_hook_existence_only() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_A);
    endpoint.add_connection(ENDPOINT_A, true);

    endpoint
        .add_inbound(echo_packet_with_end(vec![ENDPOINT_A], hook_id, true))
        .unwrap();

    let packet = single_outbound_packet(&endpoint, ENDPOINT_A);
    assert_eq!(packet.hook_id, hook_id);
    assert_hook_removed(&endpoint, hook_id);
}
