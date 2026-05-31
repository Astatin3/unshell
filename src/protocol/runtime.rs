use crate::{
    interface::InterfaceStore,
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
    packets: PacketQueue,
}

impl LeafOutbox {
    /// Creates an empty leaf-level outbox.
    pub fn new() -> Self {
        Self {
            packets: PacketQueue::new(),
        }
    }

    /// Adds one packet to the retry queue.
    pub fn push(&mut self, packet: Packet) {
        self.packets.push_back(packet);
    }

    /// Adds all packets from `packets` in FIFO order.
    pub fn extend(&mut self, packets: PacketQueue) {
        self.packets.extend(packets);
    }

    /// Returns the number of queued packets.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Returns true when the queue has no pending packets.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
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
    mut interface: Option<&mut InterfaceStore>,
) where
    S: Session<L>,
{
    let hook_id = packet.hook_id;
    let procedure_id = S::PROCEDURE_ID;

    if let Some(store) = crate::interface::borrow_store(&mut interface) {
        store.record_inbound(leaf_id, &packet);
    }

    if let Some(entry) = family
        .entries
        .iter_mut()
        .find(|entry| entry.hook_id == hook_id)
    {
        entry.inbox.push_back(packet);

        if let Some(store) = interface {
            store.record_session_packet_queued(leaf_id, procedure_id, hook_id);
        }

        return;
    }

    let started_ns = interface.as_ref().and_then(|store| store.now_ns());
    let packet_path = packet.path.clone();
    let mut init = SessionInit::new(hook_id, packet_path);

    match S::init(leaf, packet, &mut init) {
        SessionInitResult::Created(state) => {
            family.entries.push(SessionEntry::new(hook_id, state));

            if let Some(store) = interface {
                store.record_session_created(leaf_id, procedure_id, hook_id, started_ns);
            }
        }
        SessionInitResult::Rejected => {
            if let Some(store) = interface {
                store.record_session_rejected(leaf_id, procedure_id, hook_id, started_ns);
            }
        }
        SessionInitResult::RejectedWith(packet) => {
            if let Some(store) = interface {
                store.record_session_rejected(leaf_id, procedure_id, hook_id, started_ns);
                store.record_outbound_queued(leaf_id, &packet);
            }

            outbox.push(packet);
        }
    }
}

/// Updates every live session in one generated session family.
pub fn update_session_family<L, S>(
    leaf_id: u32,
    leaf: &mut L,
    family: &mut SessionFamily<S::State>,
    mut interface: Option<&mut InterfaceStore>,
) where
    S: Session<L>,
{
    for entry in &mut family.entries {
        if entry.closed {
            continue;
        }

        let started_ns = interface.as_ref().and_then(|store| store.now_ns());
        let reply_path = S::reply_path(&entry.state).to_vec();
        let mut ctx = SessionCtx::new(
            entry.hook_id,
            reply_path,
            S::PROCEDURE_ID,
            &mut entry.outbox,
        );
        let status = S::update(leaf, &mut entry.state, &mut entry.inbox, &mut ctx);

        if let Some(store) = crate::interface::borrow_store(&mut interface) {
            store.record_session_update(
                leaf_id,
                S::PROCEDURE_ID,
                entry.hook_id,
                status,
                started_ns,
            );
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
    mut interface: Option<&mut InterfaceStore>,
) where
    P: Procedure<L>,
{
    let started_ns = interface.as_ref().and_then(|store| store.now_ns());

    if let Some(store) = crate::interface::borrow_store(&mut interface) {
        store.record_inbound(leaf_id, &packet);
    }

    let hook_id = packet.hook_id;
    let mut procedure_out =
        ProcedureOut::new(hook_id, parent_reply_path(endpoint), P::PROCEDURE_ID);

    P::handle(leaf, endpoint, packet, &mut procedure_out);

    let packets = procedure_out.into_packets();

    if let Some(store) = interface {
        store.record_procedure_call(leaf_id, P::PROCEDURE_ID, hook_id, started_ns);

        for packet in &packets {
            store.record_outbound_queued(leaf_id, packet);
        }
    }

    outbox.extend(packets);
}

/// Flushes a generated leaf-level outbox through endpoint routing.
pub fn flush_leaf_outbox(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    outbox: &mut LeafOutbox,
    interface: Option<&mut InterfaceStore>,
) -> bool {
    flush_packet_queue_with_interface(endpoint, leaf_id, &mut outbox.packets, interface)
}

/// Flushes and retains one generated session family.
pub fn flush_session_family<L, S>(
    endpoint: &mut Endpoint,
    leaf_id: u32,
    family: &mut SessionFamily<S::State>,
    mut interface: Option<&mut InterfaceStore>,
) where
    S: Session<L>,
{
    for entry in &mut family.entries {
        flush_packet_queue_with_interface(
            endpoint,
            leaf_id,
            &mut entry.outbox,
            crate::interface::borrow_store(&mut interface),
        );
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
    mut interface: Option<&mut InterfaceStore>,
) -> bool {
    while let Some(packet) = outbox.front().cloned() {
        if let Some(store) = crate::interface::borrow_store(&mut interface) {
            store.record_route_attempt(leaf_id, &packet);
        }

        match endpoint.add_outbound(packet.clone()) {
            Ok(()) => {
                if let Some(store) = crate::interface::borrow_store(&mut interface) {
                    store.record_route_success(leaf_id, &packet);
                }

                outbox.pop_front();
            }
            Err(error) => {
                if let Some(store) = interface {
                    store.record_route_failure(leaf_id, &packet, error);
                }

                return false;
            }
        }
    }

    true
}

/// Returns the path used by generated procedure responses.
fn parent_reply_path(endpoint: &Endpoint) -> alloc::vec::Vec<u32> {
    if endpoint.path.len() > 1 {
        endpoint.path[..endpoint.path.len() - 1].to_vec()
    } else {
        endpoint.path.clone()
    }
}
