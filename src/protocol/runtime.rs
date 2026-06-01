use alloc::collections::VecDeque;

use crate::{
    interface::{InterfaceStore, InterfaceTarget},
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
    target: LeafOutboxTarget,
}

/// Interface owner attached to a leaf-level outbox entry.
#[derive(Clone, Copy)]
enum LeafOutboxTarget {
    /// Compatibility path for packets queued through the public `push`/`extend` API.
    InferFromPacket,

    /// Runtime-known session or procedure target.
    Explicit(InterfaceTarget),
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
        self.push_with_target(packet, LeafOutboxTarget::InferFromPacket);
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
        self.push_with_target(packet, LeafOutboxTarget::Explicit(target));
    }

    /// Adds all packets with the same runtime-known interface target.
    pub(crate) fn extend_for_target(&mut self, packets: PacketQueue, target: InterfaceTarget) {
        for packet in packets {
            self.push_for_target(packet, target);
        }
    }

    fn push_with_target(&mut self, packet: Packet, target: LeafOutboxTarget) {
        self.packets.push_back(LeafOutboxEntry { packet, target });
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
        store.record_inbound_for(target, &packet);
    }

    if let Some(entry) = family
        .entries
        .iter_mut()
        .find(|entry| entry.hook_id == hook_id)
    {
        entry.inbox.push_back(packet);

        if let Some(store) = interface.as_mut() {
            store.record_session_packet_queued_for(target);
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
                store.record_session_created_for(target, started_ns);
            }
        }
        SessionInitResult::Rejected => {
            if let Some(store) = interface.as_mut() {
                store.record_session_rejected_for(target, started_ns);
            }
        }
        SessionInitResult::RejectedWith(packet) => {
            if let Some(store) = interface.as_mut() {
                store.record_session_rejected_for(target, started_ns);
                store.record_outbound_queued_for(target, &packet);
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
            store.record_session_update_for(target, status, started_ns);

            for packet in entry.outbox.iter().skip(outbox_start) {
                store.record_outbound_queued_for(target, packet);
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
        store.record_inbound_for(target, &packet);
    }

    let hook_id = packet.hook_id;
    let mut procedure_out =
        ProcedureOut::new(hook_id, parent_reply_path(endpoint), P::PROCEDURE_ID);

    P::handle(leaf, endpoint, packet, &mut procedure_out);

    let packets = procedure_out.into_packets();

    if let Some(store) = interface.as_mut() {
        store.record_procedure_call_for(target, hook_id, started_ns);

        for packet in &packets {
            store.record_outbound_queued_for(target, packet);
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
    while let Some(entry) = outbox.packets.front().cloned() {
        let target = resolve_leaf_outbox_target(leaf_id, &entry);

        if !flush_packet_with_target(endpoint, target, &entry.packet, interface) {
            return false;
        }

        outbox.packets.pop_front();
    }

    true
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
    while let Some(packet) = outbox.front().cloned() {
        let target = InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id);

        if !flush_packet_with_target(endpoint, target, &packet, interface) {
            return false;
        }

        outbox.pop_front();
    }

    true
}

/// Flushes a packet queue whose owner is already known by the generated runtime.
fn flush_packet_queue_with_target(
    endpoint: &mut Endpoint,
    target: InterfaceTarget,
    outbox: &mut PacketQueue,
    interface: &mut Option<&mut InterfaceStore>,
) -> bool {
    while let Some(packet) = outbox.front().cloned() {
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
        store.record_route_attempt_for(target, packet);
    }

    match endpoint.add_outbound(packet.clone()) {
        Ok(()) => {
            if let Some(store) = interface.as_mut() {
                store.record_route_success_for(target, packet);
            }

            true
        }
        Err(error) => {
            if let Some(store) = interface.as_mut() {
                store.record_route_failure_for(target, packet, error);
            }

            false
        }
    }
}

fn resolve_leaf_outbox_target(leaf_id: u32, entry: &LeafOutboxEntry) -> InterfaceTarget {
    match entry.target {
        LeafOutboxTarget::InferFromPacket => {
            InterfaceTarget::session(leaf_id, entry.packet.procedure_id, entry.packet.hook_id)
        }
        LeafOutboxTarget::Explicit(target) => target,
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
