//! Core endpoint state and externally visible types.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::fmt;

use crate::protocol::{
    CallMessage, DataMessage, FaultMessage, FrameBytes, FrameError, PacketHeader, ValidationError,
};

use super::super::{CompiledRoutes, HookKey, HookTable, RouteDecision};

/// Routing metadata for one direct child endpoint.
///
/// This exists so one endpoint can distinguish topology from registration state. A child path may
/// be known structurally while still being excluded from route decisions.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ChildRoute;
/// let route = ChildRoute::registered(vec!["root".into(), "worker".into()]);
/// assert!(route.registered);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    /// Absolute path for the child endpoint inside the protocol tree.
    pub path: Vec<String>,
    /// Whether this child currently participates in routing decisions.
    pub registered: bool,
}

impl ChildRoute {
    #[must_use]
    /// Builds one child route that is immediately eligible for routing decisions.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ChildRoute;
    /// let route = ChildRoute::registered(vec!["worker".into()]);
    /// assert!(route.registered);
    /// ```
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            registered: true,
        }
    }
}

/// Procedures exposed by a named leaf attached to this endpoint.
///
/// This exists so endpoint construction can advertise one leaf's callable procedure ids up front,
/// before any runtime packets arrive.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::LeafSpec;
/// let leaf = LeafSpec {
///     name: "service".into(),
///     procedures: vec!["example.service.v1.invoke".into()],
/// };
/// assert_eq!(leaf.procedures.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    /// Leaf identifier used in packet headers.
    pub name: String,
    /// Procedures this leaf accepts.
    pub procedures: Vec<String>,
}

/// Where an inbound frame entered this endpoint.
///
/// This exists because protocol validation depends on whether a packet arrived from the parent,
/// one child subtree, or the endpoint itself.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::Ingress;
/// let ingress = Ingress::Child(vec!["root".into(), "worker".into()]);
/// assert!(matches!(ingress, Ingress::Child(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingress {
    /// The frame arrived from the parent side of the tree.
    Parent,
    /// The frame arrived from one direct child, identified by that child's absolute path.
    Child(Vec<String>),
    /// The frame originated locally at this endpoint.
    Local,
}

/// Event produced when the endpoint handles a packet locally.
///
/// This is the validated handoff boundary between transport/routing code and application-facing
/// runtimes layered on top of `ProtocolEndpoint`.
///
/// # Example
/// ```rust
/// use unshell::protocol::{CallMessage, PacketHeader, PacketType};
/// use unshell::protocol::tree::LocalEvent;
/// let event = LocalEvent::Call {
///     header: PacketHeader {
///         packet_type: PacketType::Call,
///         src_path: vec!["root".into()],
///         dst_path: vec!["worker".into()],
///         dst_leaf: None,
///         hook_id: None,
///     },
///     message: CallMessage {
///         procedure_id: "example.invoke".into(),
///         data: vec![],
///         response_hook: None,
///     },
/// };
/// assert!(matches!(event, LocalEvent::Call { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEvent {
    /// One opening `Call` packet validated and delivered to local code.
    Call {
        /// Validated protocol header for the packet.
        header: PacketHeader,
        /// Deserialized call payload.
        message: CallMessage,
    },
    /// One hook-associated `Data` packet validated and delivered locally.
    Data {
        /// Validated protocol header for the packet.
        header: PacketHeader,
        /// Deserialized data payload.
        message: DataMessage,
        /// Canonical host-scoped hook key resolved for this hook stream.
        hook_key: HookKey,
    },
    /// One hook-associated `Fault` packet validated and delivered locally.
    Fault {
        /// Validated protocol header for the packet.
        header: PacketHeader,
        /// Deserialized fault payload.
        message: FaultMessage,
        /// Canonical host-scoped hook key resolved for this hook stream.
        hook_key: HookKey,
    },
}

/// Result of processing a frame or building a locally-sent packet.
///
/// This exists so callers can distinguish forwarding, local delivery, and intentional drops
/// without treating normal protocol routing outcomes as errors.
///
/// # Example
/// ```rust
/// use unshell::protocol::FrameBytes;
/// use unshell::protocol::tree::{EndpointOutcome, RouteDecision};
/// let outcome = EndpointOutcome::Forward {
///     route: RouteDecision::Parent,
///     frame: FrameBytes::new(),
/// };
/// assert!(matches!(outcome, EndpointOutcome::Forward { .. }));
/// ```
#[derive(Debug)]
pub enum EndpointOutcome {
    /// Frame to forward, together with the next routing decision.
    Forward {
        /// The next routing decision chosen for the forwarded frame.
        route: RouteDecision,
        /// The encoded frame bytes to send along that route.
        frame: FrameBytes,
    },
    /// Locally-delivered protocol event.
    Local(LocalEvent),
    /// Packet intentionally discarded.
    Dropped,
}

/// Error surfaced while validating or encoding protocol frames.
///
/// This exists so endpoint callers can preserve the distinction between malformed wire/archive
/// data and semantic protocol invariant failures.
///
/// # Example
/// ```rust
/// use unshell::protocol::{FrameError, ValidationError};
/// use unshell::protocol::tree::EndpointError;
/// let error = EndpointError::Frame(FrameError::Truncated);
/// assert!(matches!(error, EndpointError::Frame(_)));
/// let validation = EndpointError::Validation(ValidationError::InvalidHookId);
/// assert!(matches!(validation, EndpointError::Validation(_)));
/// ```
#[derive(Debug)]
pub enum EndpointError {
    /// Framing, archive decode, or archive encode failed.
    Frame(FrameError),
    /// One protocol invariant failed validation.
    Validation(ValidationError),
}

impl fmt::Display for EndpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => write!(f, "{error}"),
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for EndpointError {}

impl From<FrameError> for EndpointError {
    fn from(value: FrameError) -> Self {
        Self::Frame(value)
    }
}

impl From<ValidationError> for EndpointError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

/// Minimal interface implemented by protocol-tree endpoints.
///
/// This exists so higher-level runtimes can depend on one small receive/path surface instead of a
/// concrete endpoint implementation.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{ChildRoute, Endpoint, Ingress, ProtocolEndpoint};
/// let endpoint = ProtocolEndpoint::new(Vec::new(), None, vec![ChildRoute::registered(vec!["worker".into()])], Vec::new());
/// assert_eq!(endpoint.path(), &Vec::<String>::new());
/// let _ = Ingress::Local;
/// ```
pub trait Endpoint {
    /// Returns this endpoint's absolute path.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ChildRoute, Endpoint, ProtocolEndpoint};
    /// let endpoint = ProtocolEndpoint::new(Vec::new(), None, vec![ChildRoute::registered(vec!["worker".into()])], Vec::new());
    /// assert!(endpoint.path().is_empty());
    /// ```
    fn path(&self) -> &[String];

    /// Processes one inbound frame from the given ingress.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::{CallMessage, PacketHeader, PacketType, encode_packet};
    /// use unshell::protocol::tree::{Endpoint, Ingress, ProtocolEndpoint};
    /// let mut endpoint = ProtocolEndpoint::new(vec!["worker".into()], Some(Vec::new()), Vec::new(), Vec::new());
    /// let frame = encode_packet(&PacketHeader {
    ///     packet_type: PacketType::Call,
    ///     src_path: Vec::new(),
    ///     dst_path: vec!["worker".into()],
    ///     dst_leaf: None,
    ///     hook_id: None,
    /// }, &CallMessage {
    ///     procedure_id: "example.invoke".into(),
    ///     data: vec![],
    ///     response_hook: None,
    /// })?;
    /// let _outcome = endpoint.receive(&Ingress::Parent, frame);
    /// # Ok::<(), unshell::protocol::FrameError>(())
    /// ```
    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

/// Runtime state for one endpoint in the protocol tree.
///
/// This exists as the central protocol node that owns route tables, local leaf metadata, and hook
/// lifecycle state for one endpoint path.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ProtocolEndpoint;
/// let endpoint = ProtocolEndpoint::new(vec!["worker".into()], Some(Vec::new()), Vec::new(), Vec::new());
/// let _ = endpoint;
/// ```
#[derive(Debug, Default)]
pub struct ProtocolEndpoint {
    pub(crate) path: Vec<String>,
    pub(crate) children: Vec<ChildRoute>,
    pub(crate) routing: CompiledRoutes,
    pub(crate) leaves: BTreeMap<String, LeafSpec>,
    pub(crate) endpoint_procedures: BTreeSet<String>,
    pub(crate) hooks: HookTable,
}
