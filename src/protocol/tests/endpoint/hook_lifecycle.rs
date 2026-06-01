use alloc::vec;

use crate::protocol::{Endpoint, EndpointError, RouteDirection};

use super::super::support::{
    assertions::{assert_hook_present, assert_hook_removed},
    endpoints::{ENDPOINT_A, ENDPOINT_B, endpoint_at, single_outbound_packet},
    packets::echo_packet_with_end,
};

#[test]
fn end_hook_removes_hook_after_packet_is_queued() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_A);
    endpoint.add_connection(ENDPOINT_A, true);

    endpoint
        .add_outbound(echo_packet_with_end(vec![ENDPOINT_A], hook_id, true))
        .unwrap();

    assert_hook_removed(&endpoint, hook_id);
    assert_eq!(
        single_outbound_packet(&endpoint, ENDPOINT_A).hook_id,
        hook_id
    );
}

#[test]
fn failed_end_hook_route_keeps_hook_state() {
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
