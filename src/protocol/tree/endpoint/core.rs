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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Unregistered,
    Registered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRoute {
    pub path: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafSpec {
    pub name: String,
    pub procedures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingress {
    Parent,
    Child(Vec<String>),
    Local,
}

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

#[derive(Debug, Default)]
pub struct EndpointOutcome {
    pub forward: Option<(RouteDecision, FrameBytes)>,
    pub event: Option<LocalEvent>,
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

pub trait Endpoint {
    fn path(&self) -> &[String];

    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

#[derive(Debug, Default)]
pub struct ProtocolEndpoint {
    pub(crate) path: Vec<String>,
    pub(crate) children: Vec<ChildRoute>,
    pub(crate) routing: CompiledRoutes,
    pub(crate) leaves: BTreeMap<String, LeafSpec>,
    pub(crate) endpoint_procedures: BTreeSet<String>,
    pub(crate) hooks: HookTable,
}
