mod streams;
mod support;

use crate::protocol::{Endpoint, EndpointError, RouteDirection};

use alloc::{boxed::Box, vec};

use support::{
    CommsLeaf, ControllerLeaf, ENDPOINT_A, ENDPOINT_B, ENDPOINT_C, ResponderLeaf,
    assert_hook_present, assert_hook_removed, echo_packet, echo_packet_with_end, endpoint_at,
    single_inbound_packet, single_outbound_packet,
};

#[test]
fn test_oneshot() {
    let (tx_a, rx_a) = crossbeam_channel::unbounded();
    let (tx_b, rx_b) = crossbeam_channel::unbounded();

    let mut endpoint_a = Endpoint::new(
        ENDPOINT_A,
        vec![
            Box::new(ControllerLeaf { has_run: false }),
            Box::new(CommsLeaf {
                tx: tx_b,
                rx: rx_a,
                remote_id: ENDPOINT_B,
                is_authority: false,
                started: false,
            }),
        ],
    );
    endpoint_a.path = vec![ENDPOINT_A];

    let mut endpoint_b = Endpoint::new(
        ENDPOINT_B,
        vec![
            Box::new(ResponderLeaf),
            Box::new(CommsLeaf {
                tx: tx_a,
                rx: rx_b,
                remote_id: ENDPOINT_A,
                is_authority: true,
                started: false,
            }),
        ],
    );
    endpoint_b.path = vec![ENDPOINT_A, ENDPOINT_B];

    // Connections are registered routing state. The comms leaves also insert them
    // during updates, but the first application packet should not depend on leaf order.
    endpoint_a.connections.insert((ENDPOINT_B, false));
    endpoint_b.connections.insert((ENDPOINT_A, true));

    // Cycle 1: A sends request to B
    endpoint_a.update();
    endpoint_b.update();

    // Cycle 2: B receives request and sends response to A
    endpoint_b.update();
    endpoint_a.update();

    // Cycle 3: A's CommsLeaf needs one more update to pull the packet from the channel
    // and put it into the inbound queue.
    endpoint_a.update();

    // Assertions on state
    assert!(
        endpoint_a.inbound.contains_key(&ENDPOINT_A),
        "Endpoint A should have received response"
    );
    assert_eq!(
        endpoint_a.inbound.get(&ENDPOINT_A).unwrap().len(),
        1,
        "Endpoint A should have exactly one packet"
    );
    let response = &endpoint_a
        .inbound
        .get(&ENDPOINT_A)
        .unwrap()
        .front()
        .unwrap();
    assert!(response.end_hook);
    assert_eq!(response.data, "ABC123".as_bytes());
    assert!(
        endpoint_b.hook_count() == 0,
        "responder hook should be cleaned after the upward response"
    );
    // assert_eq!(response.hook_id, HOOK_ECHO);
}

#[test]
fn inbound_downward_packet_for_local_endpoint_opens_hook() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.connections.insert((ENDPOINT_A, true));

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
    assert!(endpoint.outbound.is_empty());
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
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn inbound_downward_packet_routes_to_immediate_child() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.connections.insert((ENDPOINT_A, true));
    endpoint.connections.insert((ENDPOINT_C, false));

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
    assert!(!endpoint.outbound.contains_key(&ENDPOINT_A));
}

#[test]
fn outbound_downward_packet_routes_to_immediate_child() {
    let mut endpoint = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_B);
    endpoint.connections.insert((ENDPOINT_B, false));

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
    assert!(!endpoint.outbound.contains_key(&ENDPOINT_C));
}

#[test]
fn inbound_upward_packet_with_hook_routes_to_parent() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_C);
    endpoint.connections.insert((ENDPOINT_A, true));
    endpoint.connections.insert((ENDPOINT_C, false));

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
    assert!(!endpoint.outbound.contains_key(&ENDPOINT_C));
}

#[test]
fn inbound_upward_packet_without_hook_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.connections.insert((ENDPOINT_A, true));
    endpoint.connections.insert((ENDPOINT_C, false));

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
    assert!(endpoint.inbound.is_empty());
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn forged_upward_packet_with_unknown_hook_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    endpoint.accept_hook(7, ENDPOINT_C);
    endpoint.connections.insert((ENDPOINT_A, true));
    endpoint.connections.insert((ENDPOINT_C, false));

    let error = endpoint
        .add_inbound_from(ENDPOINT_C, echo_packet_with_end(vec![ENDPOINT_A], 99, true))
        .unwrap_err();

    assert!(matches!(error, EndpointError::UnknownHook { hook_id: 99 }));
    assert_hook_present(&endpoint, 7);
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn forged_sideways_packet_is_rejected_as_incorrect_path() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_A);
    endpoint.connections.insert((ENDPOINT_A, true));

    let error = endpoint
        .add_inbound_from(
            ENDPOINT_A,
            echo_packet(vec![ENDPOINT_A, ENDPOINT_C], hook_id),
        )
        .unwrap_err();

    assert!(matches!(error, EndpointError::DestinationOutsideLocalTree));
    assert_hook_present(&endpoint, hook_id);
    assert!(endpoint.inbound.is_empty());
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn malformed_frame_is_dropped_by_comms_leaf() {
    let (tx_to_endpoint, rx_for_endpoint) = crossbeam_channel::unbounded();
    let (tx_unused, _rx_unused) = crossbeam_channel::unbounded();
    let mut endpoint = Endpoint::new(
        ENDPOINT_B,
        vec![Box::new(CommsLeaf {
            tx: tx_unused,
            rx: rx_for_endpoint,
            remote_id: ENDPOINT_A,
            is_authority: true,
            started: false,
        })],
    );
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];

    tx_to_endpoint.send(vec![0, 1, 2, 3]).unwrap();
    endpoint.update();

    assert!(endpoint.inbound.is_empty());
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn malformed_frame_does_not_block_following_valid_packet() {
    let (tx_to_endpoint, rx_for_endpoint) = crossbeam_channel::unbounded();
    let (tx_unused, _rx_unused) = crossbeam_channel::unbounded();
    let hook_id = 42;
    let mut endpoint = Endpoint::new(
        ENDPOINT_B,
        vec![Box::new(CommsLeaf {
            tx: tx_unused,
            rx: rx_for_endpoint,
            remote_id: ENDPOINT_A,
            is_authority: true,
            started: false,
        })],
    );
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];

    tx_to_endpoint.send(vec![0, 1, 2, 3]).unwrap();
    tx_to_endpoint
        .send(
            echo_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id)
                .serialize()
                .unwrap(),
        )
        .unwrap();
    endpoint.update();

    let packet = single_inbound_packet(&endpoint, ENDPOINT_B);
    assert!(!packet.end_hook);
    assert_eq!(packet.hook_id, hook_id);
    assert_hook_present(&endpoint, hook_id);
}

#[test]
fn forged_frame_without_required_hook_is_dropped_by_comms_leaf() {
    let (tx_to_endpoint, rx_for_endpoint) = crossbeam_channel::unbounded();
    let (tx_unused, _rx_unused) = crossbeam_channel::unbounded();
    let mut endpoint = Endpoint::new(
        ENDPOINT_B,
        vec![Box::new(CommsLeaf {
            tx: tx_unused,
            rx: rx_for_endpoint,
            remote_id: ENDPOINT_C,
            is_authority: false,
            started: false,
        })],
    );
    endpoint.path = vec![ENDPOINT_A, ENDPOINT_B];
    endpoint.accept_hook(7, ENDPOINT_C);
    endpoint.connections.insert((ENDPOINT_A, true));

    tx_to_endpoint
        .send(
            echo_packet_with_end(vec![ENDPOINT_A], 12, true)
                .serialize()
                .unwrap(),
        )
        .unwrap();
    endpoint.update();

    assert_hook_present(&endpoint, 7);
    assert!(endpoint.inbound.is_empty());
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn upward_outbound_without_hook_is_rejected() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    endpoint.accept_hook(7, ENDPOINT_A);
    endpoint.connections.insert((ENDPOINT_A, true));

    let new_hook = endpoint.get_hook_id();

    let error = endpoint
        .add_outbound(echo_packet_with_end(vec![ENDPOINT_A], new_hook, true))
        .unwrap_err();

    assert!(matches!(
        error,
        EndpointError::UnknownHook { hook_id: observed_hook_id } if observed_hook_id == new_hook
    ));
    assert_hook_present(&endpoint, 7);
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn downward_outbound_without_hook_is_allowed() {
    let mut endpoint = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    endpoint.connections.insert((ENDPOINT_B, false));

    let new_hook = endpoint.get_hook_id();

    endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A, ENDPOINT_B], new_hook))
        .unwrap();

    assert_eq!(endpoint.outbound.get(&ENDPOINT_B).unwrap().len(), 1);
    assert_hook_present(&endpoint, new_hook);
    assert_eq!(endpoint.hook_peer(new_hook), Some(ENDPOINT_B));
}

#[test]
fn deeper_upward_route_uses_parent_as_next_hop() {
    let mut endpoint = endpoint_at(ENDPOINT_C, vec![ENDPOINT_A, ENDPOINT_B, ENDPOINT_C]);
    let new_hook = endpoint.get_hook_id();

    endpoint.accept_hook(new_hook, ENDPOINT_B);
    endpoint.connections.insert((ENDPOINT_B, true));

    endpoint
        .add_outbound(echo_packet_with_end(vec![ENDPOINT_A], new_hook, true))
        .unwrap();

    assert!(endpoint.outbound.contains_key(&ENDPOINT_B));
    assert!(!endpoint.outbound.contains_key(&ENDPOINT_A));
    assert_hook_removed(&endpoint, new_hook);
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
    assert!(endpoint.outbound.is_empty());
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
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn end_hook_removes_hook_after_packet_is_queued() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let hook_id = endpoint.get_hook_id();
    endpoint.accept_hook(hook_id, ENDPOINT_A);
    endpoint.connections.insert((ENDPOINT_A, true));

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
    assert!(endpoint.outbound.is_empty());
}

#[test]
fn inbound_without_absolute_path_is_rejected() {
    let mut endpoint = Endpoint::new(ENDPOINT_A, vec![]);

    let error = endpoint
        .add_inbound(echo_packet(vec![ENDPOINT_A], 1))
        .unwrap_err();

    assert!(matches!(error, EndpointError::EndpointPathUnset));
    assert!(endpoint.inbound.is_empty());
}

#[test]
fn outbound_without_absolute_path_is_rejected() {
    let mut endpoint = Endpoint::new(ENDPOINT_A, vec![]);

    let error = endpoint
        .add_outbound(echo_packet(vec![ENDPOINT_A], 1))
        .unwrap_err();

    assert!(matches!(error, EndpointError::EndpointPathUnset));
    assert!(endpoint.outbound.is_empty());
}
