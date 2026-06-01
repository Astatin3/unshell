use crate::protocol::{EndpointError, HookID, Packet, SessionStatus};

/// Ordered event stored by [`crate::interface::InterfaceStore`].
///
/// Events are append-only. Views store indices into this list instead of copying the
/// same packet-flow records into every renderable bucket.
pub struct InterfaceEvent {
    /// Monotonic event sequence assigned by the interface store.
    pub sequence: u64,

    /// Caller-provided timestamp, if the frontend supplied one.
    pub time_ns: Option<u64>,

    /// Leaf id that emitted or handled the event.
    pub leaf_id: u32,

    /// Detailed event payload.
    pub kind: InterfaceEventKind,
}

/// Interface-visible event emitted by generated helpers.
pub enum InterfaceEventKind {
    /// A packet was delivered to a generated leaf.
    Inbound { packet: Packet },

    /// A packet was queued into an already-live session inbox.
    SessionPacketQueued { procedure_id: u32, hook_id: HookID },

    /// A hook-backed session was created successfully.
    SessionCreated {
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },

    /// A packet could not create a new session.
    SessionRejected {
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },

    /// One live session received an update tick.
    SessionUpdated {
        procedure_id: u32,
        hook_id: HookID,
        status: SessionStatus,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },

    /// One one-shot procedure handler ran.
    ProcedureCalled {
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },

    /// A packet was emitted by leaf logic before route retry handling.
    OutboundQueued { packet: Packet },

    /// A queued outbound packet is about to enter endpoint routing.
    RouteAttempt { packet: Packet },

    /// Endpoint routing accepted a queued outbound packet.
    RouteSuccess { packet: Packet },

    /// Endpoint routing rejected a queued outbound packet.
    RouteFailure {
        packet: Packet,
        error: EndpointError,
    },
}
