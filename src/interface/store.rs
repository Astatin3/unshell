use alloc::{collections::BTreeMap, vec::Vec};

use crate::{
    interface::{
        InterfaceEvent, InterfaceEventKind, ProcedureKey, ProcedureView, SessionKey, SessionView,
        SessionViewStatus,
    },
    protocol::{EndpointError, HookID, Packet, SessionStatus},
};

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
        self.push_packet_event(
            leaf_id,
            packet,
            InterfaceEventKind::Inbound {
                packet: packet.clone(),
            },
        );
    }

    /// Records that a packet was queued for an existing session inbox.
    pub fn record_session_packet_queued(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
    ) {
        self.push_session_event(
            leaf_id,
            procedure_id,
            hook_id,
            None,
            InterfaceEventKind::SessionPacketQueued {
                procedure_id,
                hook_id,
            },
        );
    }

    /// Records successful creation of a new session state.
    pub fn record_session_created(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
    ) {
        self.push_session_event(
            leaf_id,
            procedure_id,
            hook_id,
            Some(SessionViewStatus::Running),
            InterfaceEventKind::SessionCreated {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
    }

    /// Records rejection of a packet that could not create a session.
    pub fn record_session_rejected(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
    ) {
        self.push_session_event(
            leaf_id,
            procedure_id,
            hook_id,
            Some(SessionViewStatus::Rejected),
            InterfaceEventKind::SessionRejected {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
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
        self.push_session_event(
            leaf_id,
            procedure_id,
            hook_id,
            Some(SessionViewStatus::from_session_status(status)),
            InterfaceEventKind::SessionUpdated {
                procedure_id,
                hook_id,
                status,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
    }

    /// Records one procedure call.
    pub fn record_procedure_call(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        started_ns: Option<u64>,
    ) {
        self.push_procedure_event(
            leaf_id,
            procedure_id,
            InterfaceEventKind::ProcedureCalled {
                procedure_id,
                hook_id,
                started_ns,
                finished_ns: self.now_ns,
            },
        );
    }

    /// Records a packet emitted by leaf logic before route retry handling.
    pub fn record_outbound_queued(&mut self, leaf_id: u32, packet: &Packet) {
        self.push_packet_event(
            leaf_id,
            packet,
            InterfaceEventKind::OutboundQueued {
                packet: packet.clone(),
            },
        );
    }

    /// Records a route attempt for a queued outbound packet.
    pub fn record_route_attempt(&mut self, leaf_id: u32, packet: &Packet) {
        self.push_packet_event(
            leaf_id,
            packet,
            InterfaceEventKind::RouteAttempt {
                packet: packet.clone(),
            },
        );
    }

    /// Records a successful route attempt.
    pub fn record_route_success(&mut self, leaf_id: u32, packet: &Packet) {
        self.push_packet_event(
            leaf_id,
            packet,
            InterfaceEventKind::RouteSuccess {
                packet: packet.clone(),
            },
        );
    }

    /// Records a failed route attempt without removing the packet from retry state.
    pub fn record_route_failure(&mut self, leaf_id: u32, packet: &Packet, error: EndpointError) {
        self.push_packet_event(
            leaf_id,
            packet,
            InterfaceEventKind::RouteFailure {
                packet: packet.clone(),
                error,
            },
        );
    }

    fn push_packet_event(&mut self, leaf_id: u32, packet: &Packet, kind: InterfaceEventKind) {
        let index = self.push_event(leaf_id, kind);
        self.link_packet_event(leaf_id, packet, index);
    }

    fn push_session_event(
        &mut self,
        leaf_id: u32,
        procedure_id: u32,
        hook_id: HookID,
        status: Option<SessionViewStatus>,
        kind: InterfaceEventKind,
    ) {
        let index = self.push_event(leaf_id, kind);
        let view = self.session_view_mut(leaf_id, procedure_id, hook_id);

        if let Some(status) = status {
            view.status = status;
        }

        view.events.push(index);
    }

    fn push_procedure_event(&mut self, leaf_id: u32, procedure_id: u32, kind: InterfaceEventKind) {
        let index = self.push_event(leaf_id, kind);
        self.procedure_view_mut(leaf_id, procedure_id)
            .events
            .push(index);
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
