//! Transitional packet-processing wrapper around the current protocol endpoint.
//!
//! This module is intentionally small. It gives the new runtime crate a concrete
//! bridge to the existing packet state machine while the protocol crate is split
//! into packet-only and runtime-owned layers. The wrapper does not own transport
//! handles, does not dispatch leaves, and does not make admission decisions.

use unshell_protocol::{FrameBytes, tree::Endpoint as ProtocolEndpointTrait};

pub use unshell_protocol::tree::{
    ChildRoute, EndpointError, EndpointOutcome, HookKey, Ingress, LeafSpec, LocalEvent,
    ProtocolEndpoint, RouteDecision,
};

/// Minimal packet processor used by future single-threaded runtimes.
///
/// The processor receives one frame with an already-derived ingress side and
/// returns the existing endpoint outcome. A full `NodeRuntime` should derive the
/// ingress from registered connection metadata before calling this trait.
pub trait PacketProcessor {
    /// Processes one serialized frame through protocol validation, routing, and
    /// hook-state transitions.
    fn process_frame(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

/// Runtime-owned endpoint packet state.
///
/// This is a compatibility shell around [`ProtocolEndpoint`]. It exists so new
/// runtime code can depend on `unshell_runtime::node::EndpointState` while the
/// old protocol-tree endpoint remains the source of truth for packet invariants.
#[derive(Debug, Default)]
pub struct EndpointState {
    endpoint: ProtocolEndpoint,
}

impl EndpointState {
    /// Creates a packet state wrapper from an existing protocol endpoint.
    #[must_use]
    pub const fn new(endpoint: ProtocolEndpoint) -> Self {
        Self { endpoint }
    }

    /// Creates packet state for a root-assumed endpoint.
    #[must_use]
    pub fn root(
        local_id: impl Into<alloc::string::String>,
        leaves: alloc::vec::Vec<LeafSpec>,
    ) -> Self {
        Self::new(ProtocolEndpoint::root(local_id, leaves))
    }

    /// Returns the wrapped protocol endpoint.
    #[must_use]
    pub const fn endpoint(&self) -> &ProtocolEndpoint {
        &self.endpoint
    }

    /// Returns mutable access to the wrapped protocol endpoint.
    ///
    /// This is intentionally exposed only on the transitional wrapper. New runtime
    /// code should prefer smaller methods as the endpoint state is split apart.
    #[must_use]
    pub const fn endpoint_mut(&mut self) -> &mut ProtocolEndpoint {
        &mut self.endpoint
    }

    /// Consumes the wrapper and returns the underlying protocol endpoint.
    #[must_use]
    pub fn into_endpoint(self) -> ProtocolEndpoint {
        self.endpoint
    }
}

impl PacketProcessor for EndpointState {
    fn process_frame(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError> {
        self.endpoint.receive(ingress, frame)
    }
}
