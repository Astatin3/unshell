use alloc::collections::VecDeque;

use crate::{
    interface::{InterfaceEventKind, InterfaceStore, InterfaceTarget},
    protocol::{
        Endpoint, Packet, Procedure, ProcedureOut, Session, SessionEntry, SessionFamily,
        SessionInitError, SessionStatus,
    },
};

use super::{LeafOutbox, procedure::parent_reply_path};

/// Dispatches one packet into a generated session family with interface logging.
pub fn dispatch_session_interface<L, S>(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    leaf: &mut L,
    family: &mut SessionFamily<S>,
    packet: Packet,
    interface: &mut InterfaceStore,
) where
    S: Session<L>,
{
    let hook_id = packet.hook_id;
    let procedure_id = S::PROCEDURE_ID;
    let target = InterfaceTarget::session(leaf_id, procedure_id, hook_id);

    interface.record_for(
        target,
        InterfaceEventKind::Inbound {
            packet: packet.clone(),
        },
    );

    if let Some(entry) = family
        .entries
        .iter_mut()
        .find(|entry| entry.hook_id == hook_id)
    {
        entry.inbox.push(packet);

        interface.record_for(
            target,
            InterfaceEventKind::SessionPacketQueued {
                procedure_id,
                hook_id,
            },
        );

        return;
    }

    let started_ns = interface.now_ns();
    let Ok(path) = endpoint.hook_path(hook_id) else {
        interface.record_for(
            target,
            InterfaceEventKind::SessionRejected {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: interface.now_ns(),
            },
        );

        return;
    };
    match S::init(leaf, packet) {
        Ok(state) => {
            family.entries.push(SessionEntry::new(hook_id, state));

            interface.record_for(
                target,
                InterfaceEventKind::SessionCreated {
                    procedure_id,
                    hook_id,
                    started_ns,
                    finished_ns: interface.now_ns(),
                },
            );
        }
        Err(SessionInitError::Rejected) => {
            interface.record_for(
                target,
                InterfaceEventKind::SessionRejected {
                    procedure_id,
                    hook_id,
                    started_ns,
                    finished_ns: interface.now_ns(),
                },
            );
        }
        Err(SessionInitError::Response { data, end_hook }) => {
            let packet = Packet {
                hook_id,
                end_hook,
                path,
                procedure_id,
                data,
            };

            interface.record_for(
                target,
                InterfaceEventKind::SessionRejected {
                    procedure_id,
                    hook_id,
                    started_ns,
                    finished_ns: interface.now_ns(),
                },
            );

            let _ = flush_packet_with_target(endpoint, target, &packet, interface);
        }
    }
}

/// Updates every live session in one generated session family with interface logging.
pub fn update_session_family_interface<L, S>(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    leaf: &mut L,
    family: &mut SessionFamily<S>,
    interface: &mut InterfaceStore,
) where
    S: Session<L>,
{
    for entry in &mut family.entries {
        if entry.closed {
            continue;
        }

        let started_ns = interface.now_ns();
        let status = S::update(leaf, &mut entry.state, &mut entry.inbox, endpoint);
        let target = InterfaceTarget::session(leaf_id, S::PROCEDURE_ID, entry.hook_id);

        interface.record_for(
            target,
            InterfaceEventKind::SessionUpdated {
                procedure_id: S::PROCEDURE_ID,
                hook_id: entry.hook_id,
                status,
                started_ns,
                finished_ns: interface.now_ns(),
            },
        );

        if matches!(status, SessionStatus::Closed) {
            entry.closed = true;
        }
    }

    family.entries.retain(|entry| !entry.closed);
}

/// Dispatches one packet into a generated one-shot procedure with interface logging.
pub fn dispatch_procedure_interface<L, P>(
    leaf_id: u32,
    leaf: &mut L,
    endpoint: &mut Endpoint,
    packet: Packet,
    outbox: &mut LeafOutbox,
    interface: &mut InterfaceStore,
) where
    P: Procedure<L>,
{
    let started_ns = interface.now_ns();
    let target = InterfaceTarget::procedure(leaf_id, P::PROCEDURE_ID);

    interface.record_for(
        target,
        InterfaceEventKind::Inbound {
            packet: packet.clone(),
        },
    );

    let hook_id = packet.hook_id;
    let mut procedure_out =
        ProcedureOut::new(hook_id, parent_reply_path(endpoint), P::PROCEDURE_ID);

    P::handle(leaf, endpoint, packet, &mut procedure_out);

    let packets = procedure_out.into_packets();

    interface.record_for(
        target,
        InterfaceEventKind::ProcedureCalled {
            procedure_id: P::PROCEDURE_ID,
            hook_id,
            started_ns,
            finished_ns: interface.now_ns(),
        },
    );

    for packet in &packets {
        interface.record_for(
            target,
            InterfaceEventKind::OutboundQueued {
                packet: packet.clone(),
            },
        );
    }

    outbox.extend_for_target(packets, target);
}

/// Flushes a generated leaf-level outbox through endpoint routing with interface logging.
pub fn flush_leaf_outbox_interface(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    outbox: &mut LeafOutbox,
    interface: &mut InterfaceStore,
) -> bool {
    flush_outbox(endpoint, &mut outbox.packets, interface, |entry| {
        let target = entry.target.unwrap_or_else(|| {
            InterfaceTarget::session(leaf_id, entry.packet.procedure_id, entry.packet.hook_id)
        });

        (target, entry.packet.clone())
    })
}

fn flush_outbox<T>(
    endpoint: &mut Endpoint,
    outbox: &mut VecDeque<T>,
    interface: &mut InterfaceStore,
    mut packet_for: impl FnMut(&T) -> (InterfaceTarget, Packet),
) -> bool {
    while let Some(item) = outbox.front() {
        let (target, packet) = packet_for(item);

        if !flush_packet_with_target(endpoint, target, &packet, interface) {
            return false;
        }

        outbox.pop_front();
    }

    true
}

fn flush_packet_with_target(
    endpoint: &mut Endpoint,
    target: InterfaceTarget,
    packet: &Packet,
    interface: &mut InterfaceStore,
) -> bool {
    interface.record_for(
        target,
        InterfaceEventKind::RouteAttempt {
            packet: packet.clone(),
        },
    );

    match endpoint.add_outbound(packet.clone()) {
        Ok(()) => {
            interface.record_for(
                target,
                InterfaceEventKind::RouteSuccess {
                    packet: packet.clone(),
                },
            );

            true
        }
        Err(error) => {
            interface.record_for(
                target,
                InterfaceEventKind::RouteFailure {
                    packet: packet.clone(),
                    error,
                },
            );

            false
        }
    }
}
