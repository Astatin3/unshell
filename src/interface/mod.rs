use alloc::{collections::BTreeMap, vec::Vec};

use crate::protocol::{EndpointError, HookID, Packet, SessionStatus};

/// Caller-owned view and packet-flow store for interface frontends.
///
/// Generated leaves receive a mutable reference to this store during interface-aware
/// updates. They decide which leaf/session/procedure keys to touch, but the storage
/// itself stays with the renderer or application shell so protocol state remains
/// headless and reusable.
pub struct InterfaceStore {
    next_sequence: u64,
    now_ns: Option<u64>,
    events: Vec<InterfaceEvent>,
    sessions: BTreeMap<SessionKey, SessionView>,
    procedures: BTreeMap<ProcedureKey, ProcedureView>,
}

impl InterfaceStore {
    /// Creates an empty caller-owned interface store.
    pub fn new() -> Self {
        Self {
            next_sequence: 0,
            now_ns: None,
            events: Vec::new(),
            sessions: BTreeMap::new(),
            procedures: BTreeMap::new(),
        }
    }

    /// Sets the timestamp attached to later events.
    ///
    /// The core crate stays `no_std`, so the caller supplies time from its runtime.
    /// Passing `None` keeps event ordering without pretending the protocol owns a
    /// clock.
    pub fn set_now_ns(&mut self, now_ns: Option<u64>) {
        self.now_ns = now_ns;
    }

    /// Returns the timestamp that will be attached to new events.
    pub fn now_ns(&self) -> Option<u64> {
        self.now_ns
    }

    /// Returns all recorded events in insertion order.
    pub fn events(&self) -> &[InterfaceEvent] {
        &self.events
    }

    /// Returns all session views keyed by leaf, procedure, and hook id.
    pub fn session_views(&self) -> &BTreeMap<SessionKey, SessionView> {
        &self.sessions
    }

    /// Returns all procedure views keyed by leaf and procedure id.
    pub fn procedure_views(&self) -> &BTreeMap<ProcedureKey, ProcedureView> {
        &self.procedures
    }

    /// Returns or creates the view for a hook-backed session.
    pub fn session_view_mut(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
    ) -> &mut SessionView {
        self.sessions
            .entry(SessionKey {
                leaf_id,
                procedure_id,
                hook_id,
            })
            .or_insert_with(SessionView::new)
    }

    /// Returns or creates the view for a one-shot procedure family.
    pub fn procedure_view_mut(&mut self, leaf_id: u32, procedure_id: u32) -> &mut ProcedureView {
        self.procedures
            .entry(ProcedureKey {
                leaf_id,
                procedure_id,
            })
            .or_insert_with(ProcedureView::new)
    }

    /// Records a packet delivered to a generated leaf.
    pub fn record_inbound(&mut self, leaf_id: u32, packet: &Packet) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::Inbound {
                packet: packet.clone(),
            },
        );
        self.link_packet_event(leaf_id, packet, index);
    }

    /// Records that a packet was queued for an existing session inbox.
    pub fn record_session_packet_queued(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
    ) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::SessionPacketQueued {
                procedure_id,
                hook_id,
            },
        );
        self.session_view_mut(leaf_id, procedure_id, hook_id)
            .events
            .push(index);
    }

    /// Records successful creation of a new session state.
    pub fn record_session_created(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
    ) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::SessionCreated {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
        let view = self.session_view_mut(leaf_id, procedure_id, hook_id);
        view.status = SessionViewStatus::Running;
        view.events.push(index);
    }

    /// Records rejection of a packet that could not create a session.
    pub fn record_session_rejected(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
    ) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::SessionRejected {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
        let view = self.session_view_mut(leaf_id, procedure_id, hook_id);
        view.status = SessionViewStatus::Rejected;
        view.events.push(index);
    }

    /// Records one session update tick.
    pub fn record_session_update(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        status: SessionStatus,
        started_ns: Option<u64>,
    ) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::SessionUpdated {
                procedure_id,
                hook_id,
                status,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
        let view = self.session_view_mut(leaf_id, procedure_id, hook_id);
        view.status = SessionViewStatus::from_session_status(status);
        view.events.push(index);
    }

    /// Records one procedure call.
    pub fn record_procedure_call(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
    ) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::ProcedureCalled {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
        self.procedure_view_mut(leaf_id, procedure_id)
            .events
            .push(index);
    }

    /// Records a packet emitted by leaf logic before route retry handling.
    pub fn record_outbound_queued(&mut self, leaf_id: u32, packet: &Packet) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::OutboundQueued {
                packet: packet.clone(),
            },
        );
        self.link_packet_event(leaf_id, packet, index);
    }

    /// Records a route attempt for a queued outbound packet.
    pub fn record_route_attempt(&mut self, leaf_id: u32, packet: &Packet) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::RouteAttempt {
                packet: packet.clone(),
            },
        );
        self.link_packet_event(leaf_id, packet, index);
    }

    /// Records a successful route attempt.
    pub fn record_route_success(&mut self, leaf_id: u32, packet: &Packet) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::RouteSuccess {
                packet: packet.clone(),
            },
        );
        self.link_packet_event(leaf_id, packet, index);
    }

    /// Records a failed route attempt without removing the packet from retry state.
    pub fn record_route_failure(&mut self, leaf_id: u32, packet: &Packet, error: EndpointError) {
        let index = self.push_event(
            leaf_id,
            InterfaceEventKind::RouteFailure {
                packet: packet.clone(),
                error,
            },
        );
        self.link_packet_event(leaf_id, packet, index);
    }

    fn push_event(&mut self, leaf_id: u32, kind: InterfaceEventKind) -> usize {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let index = self.events.len();

        self.events.push(InterfaceEvent {
            sequence,
            time_ns: self.now_ns,
            leaf_id,
            kind,
        });

        index
    }

    fn link_packet_event(&mut self, leaf_id: u32, packet: &Packet, index: usize) {
        self.session_view_mut(leaf_id, packet.procedure_id, packet.hook_id)
            .events
            .push(index);
    }
}

impl Default for InterfaceStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Reborrows an optional interface store for one helper call.
///
/// Generated leaf templates pass the same optional store through several helper
/// calls in one update. This small function keeps that reborrow explicit and avoids
/// every generated call site having to spell out `Option<&mut &mut T>` plumbing.
pub fn borrow_store<'a>(
    store: &'a mut Option<&mut InterfaceStore>,
) -> Option<&'a mut InterfaceStore> {
    store.as_mut().map(|store| &mut **store)
}

/// Stable identity for one generated session view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionKey {
    pub leaf_id: u32,
    pub procedure_id: u32,
    pub hook_id: HookID,
}

/// Stable identity for one generated procedure view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcedureKey {
    pub leaf_id: u32,
    pub procedure_id: u32,
}

/// Ordered event stored by [`InterfaceStore`].
pub struct InterfaceEvent {
    pub sequence: u64,
    pub time_ns: Option<u64>,
    pub leaf_id: u32,
    pub kind: InterfaceEventKind,
}

/// Interface-visible event emitted by generated helpers.
pub enum InterfaceEventKind {
    Inbound {
        packet: Packet,
    },
    SessionPacketQueued {
        procedure_id: u32,
        hook_id: HookID,
    },
    SessionCreated {
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },
    SessionRejected {
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },
    SessionUpdated {
        procedure_id: u32,
        hook_id: HookID,
        status: SessionStatus,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },
    ProcedureCalled {
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
        finished_ns: Option<u64>,
    },
    OutboundQueued {
        packet: Packet,
    },
    RouteAttempt {
        packet: Packet,
    },
    RouteSuccess {
        packet: Packet,
    },
    RouteFailure {
        packet: Packet,
        error: EndpointError,
    },
}

/// Caller-owned render view for one hook-backed session.
pub struct SessionView {
    pub status: SessionViewStatus,
    pub events: Vec<usize>,
}

impl SessionView {
    fn new() -> Self {
        Self {
            status: SessionViewStatus::Pending,
            events: Vec::new(),
        }
    }
}

/// Caller-owned render view for one one-shot procedure family.
pub struct ProcedureView {
    pub events: Vec<usize>,
}

impl ProcedureView {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}

/// Interface lifecycle state for one session view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionViewStatus {
    Pending,
    Running,
    Closing,
    Closed,
    Rejected,
}

impl SessionViewStatus {
    fn from_session_status(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Running => Self::Running,
            SessionStatus::Closing => Self::Closing,
            SessionStatus::Closed => Self::Closed,
        }
    }
}
