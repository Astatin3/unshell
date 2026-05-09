//! Transitional packet-processing wrapper around the current protocol endpoint.
//!
//! This module is intentionally small. It gives the new runtime crate a concrete
//! bridge to the existing packet state machine while the protocol crate is split
//! into packet-only and runtime-owned layers. The wrapper does not own transport
//! handles, does not dispatch leaves, and does not make admission decisions.

use unshell_protocol::{
    CallMessage, FrameBytes, PacketHeader, PacketType, tree::Endpoint as ProtocolEndpointTrait,
    validate_call, validate_header, validate_procedure_id,
};

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
#[derive(Clone, Debug, Default)]
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

    /// Returns the endpoint's current route decision for an absolute path.
    #[must_use]
    pub fn route_decision(&self, dst_path: &[alloc::string::String]) -> RouteDecision {
        self.endpoint.route_decision(dst_path)
    }

    /// Builds and routes one hook-data packet through the wrapped endpoint state.
    pub fn send_hook_data(
        &mut self,
        dst_path: alloc::vec::Vec<alloc::string::String>,
        hook_id: u64,
        procedure_id: alloc::string::String,
        data: alloc::vec::Vec<u8>,
        end_hook: bool,
    ) -> Result<EndpointOutcome, EndpointError> {
        self.endpoint
            .send_data(dst_path, hook_id, procedure_id, data, end_hook)
    }

    /// Builds and routes one call packet through the wrapped endpoint state.
    pub fn send_call(
        &mut self,
        dst_path: alloc::vec::Vec<alloc::string::String>,
        dst_leaf: Option<alloc::string::String>,
        procedure_id: alloc::string::String,
        response_hook_id: Option<u64>,
        data: alloc::vec::Vec<u8>,
    ) -> Result<EndpointOutcome, EndpointError> {
        self.endpoint
            .send_call(dst_path, dst_leaf, procedure_id, response_hook_id, data)
    }

    /// Validates an outbound call request before allocating response hook state.
    pub fn validate_call_request(
        &self,
        dst_path: &[alloc::string::String],
        dst_leaf: Option<&alloc::string::String>,
        procedure_id: &str,
        data: &[u8],
        expects_response: bool,
    ) -> Result<(), EndpointError> {
        validate_procedure_id(procedure_id)?;

        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: self.endpoint.path().to_vec(),
            dst_path: dst_path.to_vec(),
            dst_leaf: dst_leaf.cloned(),
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id: procedure_id.into(),
            data: data.to_vec(),
            response_hook: expects_response.then(|| unshell_protocol::HookTarget {
                hook_id: 1,
                return_path: self.endpoint.path().to_vec(),
            }),
        };

        validate_header(&header)?;
        validate_call(&header, &call)?;
        Ok(())
    }

    /// Allocates a response hook id scoped to this endpoint path.
    #[must_use]
    pub fn allocate_hook_id(&mut self) -> u64 {
        self.endpoint.allocate_hook_id()
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
