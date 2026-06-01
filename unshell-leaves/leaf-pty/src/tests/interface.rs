use alloc::vec;

use unshell::{
    interface::{InterfaceEventKind, InterfaceStore, ProcedureKey, SessionKey, SessionViewStatus},
    protocol::{Leaf, Packet, SessionStatus},
};

use crate::{
    FakePtyLeaf, FakePtyState, OP_TERMINATE, PROC_PTY, constants::PROC_PING, pty_open_packet,
};

use super::support::{
    ENDPOINT_A, ENDPOINT_B, drain_parent_packets, drain_parent_pty_packets, pty_endpoints,
    send_downward_frame, transfer_packets,
};

fn view_has_event<F>(interface: &InterfaceStore, event_indexes: &[usize], mut predicate: F) -> bool
where
    F: FnMut(&InterfaceEventKind) -> bool,
{
    event_indexes
        .iter()
        .any(|index| predicate(&interface.events()[*index].kind))
}

fn send_downward_ping(
    endpoint_a: &mut unshell::protocol::Endpoint,
    endpoint_b: &mut unshell::protocol::Endpoint,
    hook_id: u16,
    payload: &[u8],
) {
    endpoint_a
        .add_outbound(Packet {
            hook_id,
            end_hook: false,
            path: vec![ENDPOINT_A, ENDPOINT_B],
            procedure_id: PROC_PING,
            data: payload.to_vec(),
        })
        .unwrap();

    transfer_packets(endpoint_a, endpoint_b, ENDPOINT_B, ENDPOINT_A);
}

#[test]
fn interface_update_records_session_flow() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let mut interface = InterfaceStore::new();
    let hook_id = endpoint_a.get_hook_id();

    endpoint_a
        .add_outbound(pty_open_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id))
        .unwrap();
    transfer_packets(&mut endpoint_a, &mut endpoint_b, ENDPOINT_B, ENDPOINT_A);

    leaf.update_interface(&mut endpoint_b, &mut interface);

    let session_key = SessionKey {
        leaf_id: leaf.get_id(),
        procedure_id: PROC_PTY,
        hook_id,
    };
    let session_view = interface.session_views().get(&session_key).unwrap();

    assert_eq!(leaf.active_session_count(), 1);
    assert!(view_has_event(
        &interface,
        &session_view.events,
        |event| matches!(
            event,
            InterfaceEventKind::SessionCreated { hook_id: recorded_hook, .. }
                if *recorded_hook == hook_id
        ),
    ));
    assert!(view_has_event(
        &interface,
        &session_view.events,
        |event| matches!(
            event,
            InterfaceEventKind::SessionUpdated { hook_id: recorded_hook, status, .. }
                if *recorded_hook == hook_id && *status == SessionStatus::Running
        ),
    ));
}

#[test]
fn interface_update_records_failed_direct_route_without_retry() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let mut interface = InterfaceStore::new();
    let hook_id = endpoint_a.get_hook_id();

    endpoint_a
        .add_outbound(pty_open_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id))
        .unwrap();
    transfer_packets(&mut endpoint_a, &mut endpoint_b, ENDPOINT_B, ENDPOINT_A);
    leaf.update_interface(&mut endpoint_b, &mut interface);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_TERMINATE,
        &[],
        false,
    );
    endpoint_b.connections.remove(&(ENDPOINT_A, true));
    leaf.update_interface(&mut endpoint_b, &mut interface);

    let session_key = SessionKey {
        leaf_id: leaf.get_id(),
        procedure_id: PROC_PTY,
        hook_id,
    };
    let session_view = interface.session_views().get(&session_key).unwrap();

    assert_eq!(leaf.active_session_count(), 0);
    assert_eq!(leaf.pending_packet_count(), 0);
    assert_eq!(session_view.status, SessionViewStatus::Closed);

    endpoint_b.connections.insert((ENDPOINT_A, true));
    leaf.update_interface(&mut endpoint_b, &mut interface);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    let session_view = interface.session_views().get(&session_key).unwrap();

    assert_eq!(leaf.active_session_count(), 0);
    assert!(packets.is_empty());
    assert_eq!(session_view.status, SessionViewStatus::Closed);
}

#[test]
fn interface_update_records_procedure_flow_without_session_view() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let mut interface = InterfaceStore::new();
    let hook_id = endpoint_a.get_hook_id();

    send_downward_ping(&mut endpoint_a, &mut endpoint_b, hook_id, b"ping");
    leaf.update_interface(&mut endpoint_b, &mut interface);

    let leaf_id = leaf.get_id();
    let procedure_key = ProcedureKey {
        leaf_id,
        procedure_id: PROC_PING,
    };
    let session_key = SessionKey {
        leaf_id,
        procedure_id: PROC_PING,
        hook_id,
    };
    let procedure_view = interface.procedure_views().get(&procedure_key).unwrap();

    assert!(!interface.session_views().contains_key(&session_key));
    assert!(view_has_event(
        &interface,
        &procedure_view.events,
        |event| matches!(
            event,
            InterfaceEventKind::Inbound { packet }
                if packet.hook_id == hook_id && packet.procedure_id == PROC_PING
        ),
    ));
    assert!(view_has_event(
        &interface,
        &procedure_view.events,
        |event| matches!(
            event,
            InterfaceEventKind::ProcedureCalled { procedure_id, hook_id: recorded_hook, .. }
                if *procedure_id == PROC_PING && *recorded_hook == hook_id
        ),
    ));
    assert!(view_has_event(
        &interface,
        &procedure_view.events,
        |event| matches!(
            event,
            InterfaceEventKind::RouteSuccess { packet }
                if packet.hook_id == hook_id && packet.procedure_id == PROC_PING
        ),
    ));

    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_packets(&mut endpoint_a, PROC_PING);

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].hook_id, hook_id);
    assert!(packets[0].end_hook);
    assert_eq!(packets[0].data, b"ping".to_vec());
}
