//! Core endpoint state and externally visible types.
//!
//! This file maps to the protocol concepts described in `PROTOCOL.md`:
//! - Packet processing entry points and local delivery state: "Packet Types"
//! - Child registration state used during route selection: "Routing"
//! - Hook-hosting endpoint state: "Hooks"

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

/// Local connection state used for child route eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// The child exists in the static topology but is not currently routable.
    Unregistered,
    /// The child may receive routed traffic.
    Registered,
}

/// Child path plus current registration state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    /// Absolute child endpoint path.
    pub path: Vec<String>,
    /// Whether the child currently participates in routing.
    pub state: ConnectionState,
}

impl ChildRoute {
    /// Convenience constructor for the common registered-child case.
    #[must_use]
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            state: ConnectionState::Registered,
        }
    }
}

/// Static leaf metadata used for procedure dispatch and introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    /// Stable local leaf name.
    pub name: String,
    /// Procedures supported by the leaf.
    pub procedures: Vec<String>,
}

/// Where a frame entered the local endpoint.
///
/// This corresponds to the authority and ingress checks described in the
/// `PROTOCOL.md` routing and call sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingress {
    /// Received from the parent link.
    Parent,
    /// Received from the child at the given absolute path.
    Child(Vec<String>),
    /// Injected locally by code running on this endpoint.
    Local,
}

/// Locally delivered protocol events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEvent {
    /// A call reached this endpoint runtime.
    Call {
        header: PacketHeader,
        message: CallMessage,
    },
    /// Hook data reached this endpoint runtime.
    Data {
        header: PacketHeader,
        message: DataMessage,
    },
    /// A protocol fault reached this endpoint runtime.
    Fault {
        header: PacketHeader,
        message: FaultMessage,
    },
}

/// Result of processing one framed packet.
#[derive(Debug, Default)]
pub struct EndpointOutcome {
    /// Forwarding action to perform after local processing.
    pub forward: Option<(RouteDecision, FrameBytes)>,
    /// Event delivered to the local runtime consumer.
    pub event: Option<LocalEvent>,
    /// Whether the packet was intentionally dropped with no other side effects.
    pub dropped: bool,
}

impl EndpointOutcome {
    /// Returns an outcome that only forwards one frame.
    #[must_use]
    pub fn forward(route: RouteDecision, frame: FrameBytes) -> Self {
        Self {
            forward: Some((route, frame)),
            event: None,
            dropped: false,
        }
    }

    /// Returns an outcome that only delivers one local event.
    #[must_use]
    pub fn event(event: LocalEvent) -> Self {
        Self {
            forward: None,
            event: Some(event),
            dropped: false,
        }
    }

    /// Returns an outcome that silently drops the packet.
    #[must_use]
    pub fn dropped() -> Self {
        Self {
            forward: None,
            event: None,
            dropped: true,
        }
    }
}

/// Errors returned while decoding or validating a packet.
#[derive(Debug)]
pub enum EndpointError {
    /// The frame could not be decoded.
    Frame(FrameError),
    /// The decoded packet violated protocol invariants.
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

/// Public packet-processing trait exposed by the tree runtime.
pub trait Endpoint {
    /// Returns the absolute endpoint path.
    fn path(&self) -> &[String];

    /// Processes one incoming frame from the given ingress side.
    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

/// Stateful endpoint runtime implementing routing, hooks, and local dispatch.
#[derive(Debug, Default)]
pub struct ProtocolEndpoint {
    pub(crate) path: Vec<String>,
    pub(crate) children: Vec<ChildRoute>,
    pub(crate) routing: CompiledRoutes,
    pub(crate) leaves: BTreeMap<String, LeafSpec>,
    pub(crate) endpoint_procedures: BTreeSet<String>,
    pub(crate) hooks: HookTable,
}
