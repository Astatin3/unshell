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

use super::super::{HookTable, RouteDecision};

/// Local connection state used for child route eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Unregistered,
    Registered,
}

/// Child path plus current registration state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    pub path: Vec<String>,
    pub state: ConnectionState,
}

impl ChildRoute {
    /// Convenience constructor for the common registered-child case.
    pub fn registered(path: Vec<String>) -> Self {
        Self {
            path,
            state: ConnectionState::Registered,
        }
    }
}

/// Test leaf behavior implemented by the endpoint runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeafBehavior {
    Echo,
}

/// Static leaf metadata used for procedure dispatch and introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    pub name: String,
    pub procedures: Vec<String>,
    pub behavior: LeafBehavior,
}

/// Where a frame entered the local endpoint.
///
/// This corresponds to the authority and ingress checks described in the
/// `PROTOCOL.md` routing and call sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingress {
    Parent,
    Child(Vec<String>),
    Local,
}

/// Locally delivered protocol events.
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

/// Result of processing one framed packet.
#[derive(Debug, Default)]
pub struct EndpointOutcome {
    pub forwards: Vec<(RouteDecision, FrameBytes)>,
    pub events: Vec<LocalEvent>,
    pub dropped: bool,
}

/// Errors returned while decoding or validating a packet.
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

/// Public packet-processing trait exposed by the tree runtime.
pub trait Endpoint {
    fn path(&self) -> &[String];
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
    pub(crate) parent_path: Option<Vec<String>>,
    pub(crate) children: Vec<ChildRoute>,
    pub(crate) leaves: BTreeMap<String, LeafSpec>,
    pub(crate) endpoint_procedures: BTreeSet<String>,
    pub(crate) hooks: HookTable,
}
