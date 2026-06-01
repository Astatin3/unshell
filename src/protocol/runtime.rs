use alloc::collections::VecDeque;

use crate::{
    interface::{InterfaceEventKind, InterfaceStore, InterfaceTarget},
    protocol::{
        Endpoint, Packet, PacketQueue, Procedure, ProcedureOut, Session, SessionCtx, SessionEntry,
        SessionFamily, SessionInit, SessionInitResult, SessionStatus,
    },
};

/// Retry queue shared by generated leaves.
///
/// Sessions already own per-hook outboxes. This leaf-level queue is for rejected
/// session initialization responses and one-shot procedures, both of which need the
/// same retry semantics as session output without becoming separate framework types.
pub struct LeafOutbox {
    packets: VecDeque<LeafOutboxEntry>,
}

/// One packet retained by a leaf-level retry queue.
///
/// Session entry outboxes have an obvious owner from their surrounding session entry.
/// Leaf-level outboxes are mixed: rejected session initialization packets and one-shot
/// procedure responses both land here. Storing the owner beside the packet keeps route
/// logging precise without exposing another public queue type.
#[derive(Clone)]
struct LeafOutboxEntry {
    packet: Packet,
    target: Option<InterfaceTarget>,
}

impl LeafOutbox {
    /// Creates an empty leaf-level outbox.
    pub fn new() -> Self {
        Self {
            packets: VecDeque::new(),
        }
    }

    /// Adds one packet to the retry queue.
    pub fn push(&mut self, packet: Packet) {
        self.push_with_target(packet, None);
    }

    /// Adds all packets from `packets` in FIFO order.
    pub fn extend(&mut self, packets: PacketQueue) {
        for packet in packets {
            self.push(packet);
        }
    }

    /// Returns the number of queued packets.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Returns true when the queue has no pending packets.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Adds one packet with a runtime-known interface target.
    pub(crate) fn push_for_target(&mut self, packet: Packet, target: InterfaceTarget) {
        self.push_with_target(packet, Some(target));
    }

    fn push_with_target(&mut self, packet: Packet, target: Option<InterfaceTarget>) {
        self.packets.push_back(LeafOutboxEntry { packet, target });
    }

    /// Adds all packets with the same runtime-known interface target.
    pub(crate) fn extend_for_target(&mut self, packets: PacketQueue, target: InterfaceTarget) {
        for packet in packets {
            self.push_for_target(packet, target);
        }
    }
}

impl Default for LeafOutbox {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatches one packet into a generated session family.
///
/// The macro picks `S` and the family field. This helper owns the boring details:
/// find the hook, initialize missing sessions, queue rejected responses, and update
/// interface state when a caller supplied one.
pub fn dispatch_session<L, S>(
    leaf_id: u32,
    leaf: &mut L,
    family: &mut SessionFamily<S::State>,
    packet: Packet,
    outbox: &mut LeafOutbox,
    interface: &mut Option<&mut InterfaceStore>,
) where
    S: Session<L>,
{
    let hook_id = packet.hook_id;
    let procedure_id = S::PROCEDURE_ID;
    let target = InterfaceTarget::session(leaf_id, procedure_id, hook_id);

    if let Some(store) = interface.as_mut() {
        store.record_for(
            target,
            InterfaceEventKind::Inbound {
                packet: packet.clone(),
            },
        );
    }

    if let Some(entry) = family
        .entries
        .iter_mut()
        .find(|entry| entry.hook_id == hook_id)
    {
        entry.inbox.push_back(packet);

        if let Some(store) = interface.as_mut() {
            store.record_for(
                target,
                InterfaceEventKind::SessionPacketQueued {
                    procedure_id,
                    hook_id,
                },
            );
        }

        return;
    }

    let started_ns = interface.as_ref().and_then(|store| store.now_ns());
    let packet_path = packet.path.clone();
    let mut init = SessionInit::new(hook_id, packet_path);

    match S::init(leaf, packet, &mut init) {
        SessionInitResult::Created(state) => {
            family.entries.push(SessionEntry::new(hook_id, state));

            if let Some(store) = interface.as_mut() {
                store.record_for(
                    target,
                    InterfaceEventKind::SessionCreated {
                        procedure_id,
                        hook_id,
                        started_ns,
                        finished_ns: store.now_ns(),
                    },
                );
            }
        }
        SessionInitResult::Rejected => {
            if let Some(store) = interface.as_mut() {
                store.record_for(
                    target,
                    InterfaceEventKind::SessionRejected {
                        procedure_id,
                        hook_id,
                        started_ns,
                        finished_ns: store.now_ns(),
                    },
                );
            }
        }
        SessionInitResult::RejectedWith(packet) => {
            if let Some(store) = interface.as_mut() {
                store.record_for(
                    target,
                    InterfaceEventKind::SessionRejected {
                        procedure_id,
                        hook_id,
                        started_ns,
                        finished_ns: store.now_ns(),
                    },
                );
                store.record_for(
                    target,
                    InterfaceEventKind::OutboundQueued {
                        packet: packet.clone(),
                    },
                );
            }

            outbox.push_for_target(packet, target);
        }
    }
}

/// Updates every live session in one generated session family.
pub fn update_session_family<L, S>(
    leaf_id: u32,
    leaf: &mut L,
    family: &mut SessionFamily<S::State>,
    interface: &mut Option<&mut InterfaceStore>,
) where
    S: Session<L>,
{
    for entry in &mut family.entries {
        if entry.closed {
            continue;
        }

        let started_ns = interface.as_ref().and_then(|store| store.now_ns());
        let outbox_start = entry.outbox.len();
        let reply_path = S::reply_path(&entry.state).to_vec();
        let status = {
            let mut ctx = SessionCtx::new(
                entry.hook_id,
                reply_path,
                S::PROCEDURE_ID,
                &mut entry.outbox,
            );

            S::update(leaf, &mut entry.state, &mut entry.inbox, &mut ctx)
        };
        let target = InterfaceTarget::session(leaf_id, S::PROCEDURE_ID, entry.hook_id);

        if let Some(store) = interface.as_mut() {
            store.record_for(
                target,
                InterfaceEventKind::SessionUpdated {
                    procedure_id: S::PROCEDURE_ID,
                    hook_id: entry.hook_id,
                    status,
                    started_ns,
                    finished_ns: store.now_ns(),
                },
            );

            for packet in entry.outbox.iter().skip(outbox_start) {
                store.record_for(
                    target,
                    InterfaceEventKind::OutboundQueued {
                        packet: packet.clone(),
                    },
                );
            }
        }

        if matches!(status, SessionStatus::Closed) {
            entry.closed = true;
        }
    }
}

/// Dispatches one packet into a generated one-shot procedure.
pub fn dispatch_procedure<L, P>(
    leaf_id: u32,
    leaf: &mut L,
    endpoint: &mut Endpoint,
    packet: Packet,
    outbox: &mut LeafOutbox,
    interface: &mut Option<&mut InterfaceStore>,
) where
    P: Procedure<L>,
{
    let started_ns = interface.as_ref().and_then(|store| store.now_ns());
    let target = InterfaceTarget::procedure(leaf_id, P::PROCEDURE_ID);

    if let Some(store) = interface.as_mut() {
        store.record_for(
            target,
            InterfaceEventKind::Inbound {
                packet: packet.clone(),
            },
        );
    }

    let hook_id = packet.hook_id;
    let mut procedure_out =
        ProcedureOut::new(hook_id, parent_reply_path(endpoint), P::PROCEDURE_ID);

    P::handle(leaf, endpoint, packet, &mut procedure_out);

    let packets = procedure_out.into_packets();

    if let Some(store) = interface.as_mut() {
        store.record_for(
            target,
            InterfaceEventKind::ProcedureCalled {
                procedure_id: P::PROCEDURE_ID,
                hook_id,
                started_ns,
                finished_ns: store.now_ns(),
            },
        );

        for packet in &packets {
            store.record_for(
                target,
                InterfaceEventKind::OutboundQueued {
                    packet: packet.clone(),
                },
            );
        }
    }

    outbox.extend_for_target(packets, target);
}

/// Flushes a generated leaf-level outbox through endpoint routing.
pub fn flush_leaf_outbox(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    outbox: &mut LeafOutbox,
    interface: &mut Option<&mut InterfaceStore>,
) -> bool {
    flush_outbox(endpoint, &mut outbox.packets, interface, |entry| {
        let target = entry.target.unwrap_or_else(|| {
            InterfaceTarget::session(leaf_id, entry.packet.procedure_id, entry.packet.hook_id)
        });

        (target, entry.packet.clone())
    })
}

/// Flushes and retains one generated session family.
pub fn flush_session_family<L, S>(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    family: &mut SessionFamily<S::State>,
    interface: &mut Option<&mut InterfaceStore>,
) where
    S: Session<L>,
{
    for entry in &mut family.entries {
        let target = InterfaceTarget::session(leaf_id, S::PROCEDURE_ID, entry.hook_id);
        flush_packet_queue_with_target(endpoint, target, &mut entry.outbox, interface);
    }

    family
        .entries
        .retain(|entry| !entry.closed || !entry.outbox.is_empty());
}

/// Flushes a retry queue through [`Endpoint::add_outbound`].
///
/// This is the interface-aware version of [`crate::protocol::flush_packet_queue`]. It
/// logs route attempts before trying them, then logs either success or the route error
/// without dropping the packet on failure.
pub fn flush_packet_queue_with_interface(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    outbox: &mut PacketQueue,
    interface: &mut Option<&mut InterfaceStore>,
) -> bool {
    flush_outbox(endpoint, outbox, interface, |packet| {
        (
            InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id),
            packet.clone(),
        )
    })
}

/// Flushes a packet queue whose owner is already known by the generated runtime.
fn flush_packet_queue_with_target(
    endpoint: &mut Endpoint,
    target: InterfaceTarget,
    outbox: &mut PacketQueue,
    interface: &mut Option<&mut InterfaceStore>,
) -> bool {
    flush_outbox(endpoint, outbox, interface, |packet| {
        (target, packet.clone())
    })
}

fn flush_outbox<T>(
    endpoint: &mut Endpoint,
    outbox: &mut VecDeque<T>,
    interface: &mut Option<&mut InterfaceStore>,
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
    interface: &mut Option<&mut InterfaceStore>,
) -> bool {
    if let Some(store) = interface.as_mut() {
        store.record_for(
            target,
            InterfaceEventKind::RouteAttempt {
                packet: packet.clone(),
            },
        );
    }

    match endpoint.add_outbound(packet.clone()) {
        Ok(()) => {
            if let Some(store) = interface.as_mut() {
                store.record_for(
                    target,
                    InterfaceEventKind::RouteSuccess {
                        packet: packet.clone(),
                    },
                );
            }

            true
        }
        Err(error) => {
            if let Some(store) = interface.as_mut() {
                store.record_for(
                    target,
                    InterfaceEventKind::RouteFailure {
                        packet: packet.clone(),
                        error,
                    },
                );
            }

            false
        }
    }
}

/// Returns the path used by generated procedure responses.
fn parent_reply_path(endpoint: &Endpoint) -> alloc::vec::Vec<u32> {
    if endpoint.path.len() > 1 {
        endpoint.path[..endpoint.path.len() - 1].to_vec()
    } else {
        endpoint.path.clone()
    }
}
