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
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            registered: true,
        }
    }
}

/// Procedures exposed by a named leaf attached to this endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    /// Leaf identifier used in packet headers.
    pub name: String,
    /// Procedures this leaf accepts.
    pub procedures: Vec<String>,
}

/// Where an inbound frame entered this endpoint.
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
#[derive(Debug)]
pub enum EndpointOutcome {
    /// Frame to forward, together with the next routing decision.
    Forward {
        route: RouteDecision,
        frame: FrameBytes,
    },
    /// Locally-delivered protocol event.
    Local(LocalEvent),
    /// Packet intentionally discarded.
    Dropped,
}

/// Error surfaced while validating or encoding protocol frames.
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
pub trait Endpoint {
    /// Returns this endpoint's absolute path.
    fn path(&self) -> &[String];

    /// Processes one inbound frame from the given ingress.
    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

/// Runtime state for one endpoint in the protocol tree.
#[derive(Debug, Default)]
pub struct ProtocolEndpoint {
    pub(crate) path: Vec<String>,
    pub(crate) children: Vec<ChildRoute>,
    pub(crate) routing: CompiledRoutes,
    pub(crate) leaves: BTreeMap<String, LeafSpec>,
    pub(crate) endpoint_procedures: BTreeSet<String>,
    pub(crate) hooks: HookTable,
}
