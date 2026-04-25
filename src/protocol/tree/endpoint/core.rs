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

use super::super::{CompiledRoutes, HookTable, RouteDecision};

/// Registration state for a direct child endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Unregistered,
    Registered,
}

/// Routing metadata for one direct child endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    /// Absolute path for the child endpoint inside the protocol tree.
    pub path: Vec<String>,
    /// Whether this child currently participates in routing decisions.
    pub state: ConnectionState,
}

impl ChildRoute {
    #[must_use]
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            state: ConnectionState::Registered,
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
    Parent,
    Child(Vec<String>),
    Local,
}

/// Event produced when the endpoint handles a packet locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEvent {
    Call {
        header: PacketHeader,
        message: CallMessage,
    },
    Data {
        header: PacketHeader,
        message: DataMessage,
    },
    Fault {
        header: PacketHeader,
        message: FaultMessage,
    },
}

/// Result of processing a frame or building a locally-sent packet.
#[derive(Debug, Default)]
pub struct EndpointOutcome {
    /// Frame to forward, together with the next routing decision.
    pub forward: Option<(RouteDecision, FrameBytes)>,
    /// Locally-delivered protocol event.
    pub event: Option<LocalEvent>,
    /// Whether the packet was intentionally discarded.
    pub dropped: bool,
}

impl EndpointOutcome {
    #[must_use]
    pub fn forward(route: RouteDecision, frame: FrameBytes) -> Self {
        Self {
            forward: Some((route, frame)),
            event: None,
            dropped: false,
        }
    }

    #[must_use]
    pub fn event(event: LocalEvent) -> Self {
        Self {
            forward: None,
            event: Some(event),
            dropped: false,
        }
    }

    #[must_use]
    pub fn dropped() -> Self {
        Self {
            forward: None,
            event: None,
            dropped: true,
        }
    }
}

/// Error surfaced while validating or encoding protocol frames.
#[derive(Debug)]
pub enum EndpointError {
    Frame(FrameError),
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
