use alloc::{collections::BTreeMap, vec::Vec};

use crate::{
    interface::{
        InterfaceEvent, InterfaceEventKind, ProcedureKey, ProcedureView, SessionKey, SessionView,
        SessionViewStatus,
    },
    protocol::{EndpointError, HookID, Packet, SessionStatus},
};

/// Internal owner for one interface event.
///
/// The runtime already knows whether a packet belongs to a hook-backed session or a
/// one-shot procedure. Keeping that answer explicit avoids reconstructing ownership
/// from packet fields later, which is what made procedure packet flow look like fake
/// session activity in the previous store implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterfaceTarget {
    /// Event belongs to one hook-backed session instance.
    Session(SessionKey),

    /// Event belongs to one one-shot procedure family.
    Procedure(ProcedureKey),
}

impl InterfaceTarget {
    /// Builds a session target from the same pieces exposed by [`SessionKey`].
    pub(crate) fn session(leaf_id: u32, procedure_id: u32, hook_id: HookID) -> Self {
        Self::Session(SessionKey {
            leaf_id,
            procedure_id,
            hook_id,
        })
    }

    /// Builds a procedure target from the same pieces exposed by [`ProcedureKey`].
    pub(crate) fn procedure(leaf_id: u32, procedure_id: u32) -> Self {
        Self::Procedure(ProcedureKey {
            leaf_id,
            procedure_id,
        })
    }

    /// Returns the leaf id used on the append-only event record.
    pub(crate) fn leaf_id(self) -> u32 {
        match self {
            Self::Session(key) => key.leaf_id,
            Self::Procedure(key) => key.leaf_id,
        }
    }
}

/// Caller-owned view and packet-flow store for interface frontends.
///
/// Generated leaves receive a mutable reference to this store during interface-aware
/// updates. They decide which leaf/session/procedure keys to touch, but the storage
/// itself stays with the renderer or application shell so protocol state remains
/// headless and reusable.
pub struct InterfaceStore {
    now_ns: Option<u64>,
    events: Vec<InterfaceEvent>,
    sessions: BTreeMap<SessionKey, SessionView>,
    procedures: BTreeMap<ProcedureKey, ProcedureView>,
}

impl InterfaceStore {
    /// Creates an empty caller-owned interface store.
    pub fn new() -> Self {
        Self {
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
        self.session_view_for_key_mut(SessionKey {
            leaf_id,
            procedure_id,
            hook_id,
        })
    }

    /// Returns or creates the view for a one-shot procedure family.
    pub fn procedure_view_mut(&mut self, leaf_id: u32, procedure_id: u32) -> &mut ProcedureView {
        self.procedure_view_for_key_mut(ProcedureKey {
            leaf_id,
            procedure_id,
        })
    }

    /// Records a packet delivered to a generated leaf.
    pub fn record_inbound(&mut self, leaf_id: u32, packet: &Packet) {
        let target = InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id);
        self.record_for(
            target,
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
        self.record_for(
            InterfaceTarget::session(leaf_id, procedure_id, hook_id),
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
        self.record_for(
            InterfaceTarget::session(leaf_id, procedure_id, hook_id),
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
        self.record_for(
            InterfaceTarget::session(leaf_id, procedure_id, hook_id),
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
        self.record_for(
            InterfaceTarget::session(leaf_id, procedure_id, hook_id),
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
        self.record_for(
            InterfaceTarget::procedure(leaf_id, procedure_id),
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
        let target = InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id);
        self.record_for(
            target,
            InterfaceEventKind::OutboundQueued {
                packet: packet.clone(),
            },
        );
    }

    /// Records a route attempt for a queued outbound packet.
    pub fn record_route_attempt(&mut self, leaf_id: u32, packet: &Packet) {
        let target = InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id);
        self.record_for(
            target,
            InterfaceEventKind::RouteAttempt {
                packet: packet.clone(),
            },
        );
    }

    /// Records a successful route attempt.
    pub fn record_route_success(&mut self, leaf_id: u32, packet: &Packet) {
        let target = InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id);
        self.record_for(
            target,
            InterfaceEventKind::RouteSuccess {
                packet: packet.clone(),
            },
        );
    }

    /// Records a failed route attempt without removing the packet from retry state.
    pub fn record_route_failure(&mut self, leaf_id: u32, packet: &Packet, error: EndpointError) {
        let target = InterfaceTarget::session(leaf_id, packet.procedure_id, packet.hook_id);
        self.record_for(
            target,
            InterfaceEventKind::RouteFailure {
                packet: packet.clone(),
                error,
            },
        );
    }

    pub(crate) fn record_for(&mut self, target: InterfaceTarget, kind: InterfaceEventKind) {
        let index = self.push_event(target.leaf_id(), kind);
        self.link_event(target, index);
    }

    fn link_event(&mut self, target: InterfaceTarget, index: usize) {
        let status = Self::status_for_event(&self.events[index].kind);

        match target {
            InterfaceTarget::Session(key) => {
                let view = self.session_view_for_key_mut(key);

                if let Some(status) = status {
                    view.status = status;
                }

                view.events.push(index);
            }
            InterfaceTarget::Procedure(key) => {
                self.procedure_view_for_key_mut(key).events.push(index);
            }
        }
    }

    fn status_for_event(kind: &InterfaceEventKind) -> Option<SessionViewStatus> {
        match kind {
            InterfaceEventKind::SessionCreated { .. } => Some(SessionViewStatus::Running),
            InterfaceEventKind::SessionRejected { .. } => Some(SessionViewStatus::Rejected),
            InterfaceEventKind::SessionUpdated { status, .. } => {
                Some(SessionViewStatus::from_session_status(*status))
            }
            _ => None,
        }
    }

    fn push_event(&mut self, leaf_id: u32, kind: InterfaceEventKind) -> usize {
        let index = self.events.len();

        self.events.push(InterfaceEvent {
            sequence: index as u64,
            time_ns: self.now_ns,
            leaf_id,
            kind,
        });

        index
    }

    fn session_view_for_key_mut(&mut self, key: SessionKey) -> &mut SessionView {
        self.sessions.entry(key).or_insert_with(SessionView::new)
    }

    fn procedure_view_for_key_mut(&mut self, key: ProcedureKey) -> &mut ProcedureView {
        self.procedures
            .entry(key)
            .or_insert_with(ProcedureView::new)
    }
}

impl Default for InterfaceStore {
    fn default() -> Self {
        Self::new()
    }
}
